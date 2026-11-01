use axum::{
    body::{Body, Bytes, HttpBody},
    http::{Request, StatusCode, header},
};
use recastpress_core::{
    publication::{PreparedSite, PublicPage, RasterImageKind},
    resource_budget::{BudgetPool, ResourceKind},
};
use recastpress_server::{public_router, public_router_with_budgeted_cache};
use std::{future::poll_fn, pin::Pin, sync::Arc};
use tower::ServiceExt;

fn request(method: &str, range: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri("/")
        .header(header::HOST, "example.test");
    if let Some(range) = range {
        builder = builder.header(header::RANGE, range);
    }
    builder.body(Body::empty()).unwrap()
}

fn site(page: &PublicPage) -> PreparedSite {
    PreparedSite::new([("/".into(), page.clone())]).unwrap()
}

async fn data(mut body: Body) -> Bytes {
    // Observe the actual frame before any collector could copy its payload.
    let frame = poll_fn(|context| Pin::new(&mut body).poll_frame(context))
        .await
        .unwrap()
        .unwrap();
    let bytes = frame.into_data().unwrap();
    assert!(
        poll_fn(|context| Pin::new(&mut body).poll_frame(context))
            .await
            .is_none()
    );
    bytes
}

#[tokio::test]
async fn uncached_router_body_shares_the_sealed_payload_and_outlives_its_generation() {
    let page = PublicPage::new(
        200,
        "text/plain; charset=utf-8".into(),
        "public 日本\n".as_bytes().to_vec(),
    )
    .unwrap();
    let owner = page.shared_body();
    let weak = Arc::downgrade(&owner);
    let pointer = owner.as_ptr();
    drop(owner);
    let response = public_router(Some(site(&page)), "example.test")
        .unwrap()
        .oneshot(request("GET", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "14");
    drop(page);
    assert!(weak.upgrade().is_some()); // The unpolled response owns the payload.
    let bytes = data(response.into_body()).await;
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(bytes, "public 日本\n");
    let clone = bytes.clone();
    let slice = bytes.slice(7..13);
    drop(bytes);
    drop(clone);
    assert!(weak.upgrade().is_some());
    assert_eq!(slice, "日本");
    drop(slice);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn binary_range_frame_shares_the_original_offset_and_keeps_the_full_owner_alive() {
    let mut raw = b"\x89PNG\r\n\x1a\n".to_vec();
    raw.extend(0..=255);
    let page = PublicPage::new_image(200, RasterImageKind::Png, raw).unwrap();
    let owner = page.shared_body();
    let weak = Arc::downgrade(&owner);
    let pointer = owner[8..12].as_ptr();
    drop(owner);
    let response = public_router(Some(site(&page)), "example.test")
        .unwrap()
        .oneshot(request("GET", Some("bytes=8-11")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "4");
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 8-11/264");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert!(response.headers().contains_key(header::ETAG));
    drop(page);
    assert!(weak.upgrade().is_some());
    let bytes = data(response.into_body()).await;
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(&bytes[..], &[0, 1, 2, 3]);
    assert_eq!(weak.upgrade().unwrap().len(), 264);
    drop(bytes);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn range_bypasses_a_warm_cache_and_has_no_cache_lease_after_purge() {
    let page = PublicPage::new(
        200,
        "text/plain; charset=utf-8".into(),
        b"0123456789".to_vec(),
    )
    .unwrap();
    let source = page.shared_body();
    let weak = Arc::downgrade(&source);
    let pointer = source[1..3].as_ptr();
    drop(source);
    let pool = BudgetPool::new(16384);
    let (router, control) =
        public_router_with_budgeted_cache(Some(site(&page)), "example.test", pool.clone()).unwrap();
    assert!(control.apply_allocation(4096));
    assert!(control.reconciliation_completed());
    let cached_response = router.clone().oneshot(request("GET", None)).await.unwrap();
    let tag = cached_response.headers()[header::ETAG].clone();
    let cached_bytes = data(cached_response.into_body()).await;
    assert_eq!(control.status().unwrap().entries, 1);
    let charged = pool
        .stats()
        .unwrap()
        .claimed_for(ResourceKind::ResponseCache);
    assert!(charged > 0);
    let partial = router
        .clone()
        .oneshot(request("GET", Some("bytes=1-2")))
        .await
        .unwrap();
    assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(partial.headers()[header::ETAG], tag);
    let bytes = data(partial.into_body()).await;
    assert_eq!(bytes.as_ptr(), pointer);
    assert_ne!(bytes.as_ptr(), cached_bytes.as_ptr().wrapping_add(1));
    assert_eq!(bytes, "12");
    assert_eq!(pool.stats().unwrap().claimed, charged);
    drop(cached_bytes);
    assert!(control.purge());
    assert_eq!(pool.stats().unwrap().claimed, 0);
    drop(router);
    drop(control);
    drop(page);
    assert!(weak.upgrade().is_some());
    drop(bytes);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn empty_and_head_responses_do_not_need_to_retain_a_sealed_body_owner() {
    for (raw, method) in [(b"".as_slice(), "GET"), (b"nonempty".as_slice(), "HEAD")] {
        let page = PublicPage::new(200, "text/plain; charset=utf-8".into(), raw.to_vec()).unwrap();
        let owner = page.shared_body();
        let weak = Arc::downgrade(&owner);
        drop(owner);
        let response = public_router(Some(site(&page)), "example.test")
            .unwrap()
            .oneshot(request(method, None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            raw.len().to_string()
        );
        assert!(response.body().is_end_stream());
        drop(page);
        assert!(weak.upgrade().is_none());
    }
}

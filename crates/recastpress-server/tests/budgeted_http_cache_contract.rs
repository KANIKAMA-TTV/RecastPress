use axum::{
    body::{Body, HttpBody, to_bytes},
    http::{Request, StatusCode, header},
};
use recastpress_core::{
    cache::{MonitorState, StopReason},
    publication::{PreparedSite, PublicPage},
    resource_budget::{BudgetPool, ResourceKind},
};
use recastpress_server::{
    feeds::{FeedDocument, FeedFormat, UtcTimestamp, render_budgeted},
    http_cache::PublicCacheControl,
    public_router_with_budgeted_cache, public_router_with_cache,
};
use tower::ServiceExt;

fn site(rows: &[(&str, u16, &[u8])]) -> PreparedSite {
    PreparedSite::new(rows.iter().map(|(target, status, bytes)| {
        (
            (*target).into(),
            PublicPage::new(*status, "text/plain; charset=utf-8".into(), bytes.to_vec()).unwrap(),
        )
    }))
    .unwrap()
}

fn app(pool: BudgetPool, rows: &[(&str, u16, &[u8])]) -> (axum::Router, PublicCacheControl) {
    public_router_with_budgeted_cache(Some(site(rows)), "example.test", pool).unwrap()
}

fn request(method: &str, target: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(target)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap()
}

fn enable(control: &PublicCacheControl) {
    assert!(control.apply_allocation(65536));
    assert!(control.reconciliation_completed());
}

fn claimed(pool: &BudgetPool) -> usize {
    pool.stats()
        .unwrap()
        .claimed_for(ResourceKind::ResponseCache)
}

#[tokio::test]
async fn opt_in_starts_suspended_and_local_allocation_never_changes_shared_pool_limit() {
    let pool = BudgetPool::new(8192);
    let (app, control) = app(pool.clone(), &[("/", 200, b"public")]);
    assert_eq!(
        control.status().unwrap().stop_reason,
        Some(StopReason::MonitorNotReady)
    );
    assert!(control.reconciliation_completed());
    assert_eq!(
        control.status().unwrap().stop_reason,
        Some(StopReason::NoBudget)
    );
    let response = app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(to_bytes(response.into_body(), 100).await.unwrap(), "public");
    assert_eq!(claimed(&pool), 0);
    assert!(control.apply_allocation(65536));
    assert_eq!(pool.stats().unwrap().limit, 8192);
    let response = app.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    drop(response);
    assert_eq!(claimed(&pool), control.status().unwrap().charged_bytes);
    assert!(claimed(&pool) > 0);
    control.purge();
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn unpolled_responses_keep_one_entry_lease_after_purge_until_the_last_owner_drops() {
    let pool = BudgetPool::new(65536);
    let (app, control) = app(pool.clone(), &[("/", 200, b"public")]);
    enable(&control);
    let first = app.clone().oneshot(request("GET", "/")).await.unwrap();
    let charge = claimed(&pool);
    assert!(charge > 6);
    let second = app.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(claimed(&pool), charge);
    assert!(control.purge());
    assert_eq!(control.status().unwrap().entries, 0);
    assert_eq!(claimed(&pool), charge);
    drop(first); // Neither response body has been polled.
    assert_eq!(claimed(&pool), charge);
    drop(second);
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn collected_bytes_clones_and_nonempty_slices_preserve_the_original_owner_lease() {
    let pool = BudgetPool::new(65536);
    let (app, control) = app(pool.clone(), &[("/", 200, b"public")]);
    enable(&control);
    let response = app.oneshot(request("GET", "/")).await.unwrap();
    let charge = claimed(&pool);
    control.purge();
    let bytes = to_bytes(response.into_body(), 100).await.unwrap();
    assert_eq!(bytes, "public");
    assert_eq!(claimed(&pool), charge);
    let cloned = bytes.clone();
    let slice = bytes.slice(1..4);
    assert_eq!(cloned.as_ptr(), bytes.as_ptr());
    drop(bytes);
    drop(cloned);
    assert_eq!(slice, "ubl");
    assert_eq!(claimed(&pool), charge);
    let empty = slice.slice(0..0);
    drop(slice);
    // A zero-length slice has no owner-backed region in locked bytes1.12.1.
    assert!(empty.is_empty());
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn empty_full_body_optimization_does_not_promise_a_lease_until_empty_body_drop() {
    let pool = BudgetPool::new(65536);
    let (app, control) = app(pool.clone(), &[("/", 200, b"")]);
    enable(&control);
    let response = app.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
    assert!(claimed(&pool) > 0); // The resident entry still owns its full charge.
    let body = response.into_body();
    assert!(body.is_end_stream());
    control.purge();
    assert_eq!(claimed(&pool), 0); // Full::new discarded the empty Bytes owner.
    assert!(to_bytes(body, 0).await.unwrap().is_empty());
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn collection_limit_error_drops_the_frame_owner_and_returns_its_charge() {
    let pool = BudgetPool::new(65536);
    let (app, control) = app(pool.clone(), &[("/", 200, b"public")]);
    enable(&control);
    let response = app.oneshot(request("GET", "/")).await.unwrap();
    let charge = claimed(&pool);
    control.purge();
    assert_eq!(claimed(&pool), charge);
    assert!(to_bytes(response.into_body(), 5).await.is_err());
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn shared_admission_refusal_falls_back_without_stopping_cache_or_replacing_old_data() {
    let pool = BudgetPool::new(0);
    let (app, control) = app(pool.clone(), &[("/a", 200, b"a"), ("/b", 200, b"b")]);
    enable(&control);
    let refused = app.clone().oneshot(request("GET", "/a")).await.unwrap();
    assert_eq!(refused.status(), StatusCode::OK);
    assert_eq!(to_bytes(refused.into_body(), 10).await.unwrap(), "a");
    assert_eq!(control.status().unwrap().entries, 0);
    assert_eq!(claimed(&pool), 0);
    pool.set_limit(65536).unwrap();
    drop(app.clone().oneshot(request("GET", "/a")).await.unwrap());
    let charge = claimed(&pool);
    pool.set_limit(charge).unwrap();
    let before = control.status().unwrap();
    let refused = app.clone().oneshot(request("GET", "/b")).await.unwrap();
    assert_eq!(to_bytes(refused.into_body(), 10).await.unwrap(), "b");
    assert_eq!(control.status().unwrap(), before);
    assert_eq!(claimed(&pool), charge);
    let old = app.oneshot(request("GET", "/a")).await.unwrap();
    assert_eq!(to_bytes(old.into_body(), 10).await.unwrap(), "a");
    assert_eq!(control.status().unwrap().monitor, MonitorState::Tracking);
    control.purge();
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn monitor_loss_purges_new_hits_but_cannot_revoke_an_already_retained_response() {
    let pool = BudgetPool::new(65536);
    let (app, control) = app(pool.clone(), &[("/", 200, b"public")]);
    enable(&control);
    let retained = app.clone().oneshot(request("GET", "/")).await.unwrap();
    let charge = claimed(&pool);
    pool.set_limit(0).unwrap();
    assert_eq!(pool.stats().unwrap().debt, charge);
    assert!(control.monitoring_lost());
    assert_eq!(control.status().unwrap().monitor, MonitorState::Unavailable);
    assert_eq!(control.status().unwrap().entries, 0);
    assert_eq!(claimed(&pool), charge);
    let fallback = app.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(to_bytes(fallback.into_body(), 100).await.unwrap(), "public");
    assert_eq!(control.status().unwrap().entries, 0);
    let bytes = to_bytes(retained.into_body(), 100).await.unwrap();
    assert_eq!(bytes, "public"); // No transport-revocation guarantee is claimed.
    assert_eq!(claimed(&pool), charge);
    drop(bytes);
    assert_eq!(claimed(&pool), 0);
    assert_eq!(pool.stats().unwrap().debt, 0);
}

#[tokio::test]
async fn eligibility_host_head_and_unknown_query_policy_are_unchanged_in_opt_in_backend() {
    let pool = BudgetPool::new(65536);
    let (app, control) = app(
        pool.clone(),
        &[
            ("/", 200, b"public"),
            ("/?token=secret", 200, b"query"),
            ("/gone", 410, b"gone"),
        ],
    );
    enable(&control);
    for (method, target, status) in [
        ("HEAD", "/", StatusCode::OK),
        ("GET", "/?token=secret", StatusCode::OK),
        ("GET", "/gone", StatusCode::GONE),
        ("GET", "/missing", StatusCode::NOT_FOUND),
    ] {
        let response = app.clone().oneshot(request(method, target)).await.unwrap();
        assert_eq!(response.status(), status);
        if method == "HEAD" {
            assert!(
                to_bytes(response.into_body(), 100)
                    .await
                    .unwrap()
                    .is_empty()
            );
        } else {
            drop(response);
        }
        assert_eq!(control.status().unwrap().entries, 0);
        assert_eq!(claimed(&pool), 0);
    }
    for name in ["authorization", "cookie", "x-custom-identity"] {
        let mut req = request("GET", "/");
        req.headers_mut().insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            "private".parse().unwrap(),
        );
        drop(app.clone().oneshot(req).await.unwrap());
        assert_eq!(control.status().unwrap().entries, 0);
    }
    let mut wrong_host = request("GET", "/");
    wrong_host
        .headers_mut()
        .insert(header::HOST, "other.test".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(wrong_host).await.unwrap().status(),
        StatusCode::MISDIRECTED_REQUEST
    );
    assert_eq!(claimed(&pool), 0);
    drop(app.oneshot(request("GET", "/")).await.unwrap());
    assert_eq!(control.status().unwrap().entries, 1);
    assert!(claimed(&pool) > 0);
    control.purge();
    assert_eq!(claimed(&pool), 0);
}

#[tokio::test]
async fn legacy_constructor_remains_legacy_and_router_validation_is_shared() {
    let (legacy, control) =
        public_router_with_cache(Some(site(&[("/", 200, b"public")])), "example.test").unwrap();
    enable(&control);
    drop(legacy.oneshot(request("GET", "/")).await.unwrap());
    assert_eq!(control.status().unwrap().entries, 1);
    let pool = BudgetPool::new(0);
    for host in [
        "",
        "user@example.test",
        "example.test:65536",
        "example.test ",
    ] {
        assert!(public_router_with_cache(None, host).is_err());
        assert!(public_router_with_budgeted_cache(None, host, pool.clone()).is_err());
    }
    assert!(
        public_router_with_budgeted_cache(
            Some(site(&[("/health/live", 200, b"conflict")])),
            "example.test",
            pool.clone()
        )
        .is_err()
    );
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[tokio::test]
async fn prepared_feed_and_http_body_compete_in_one_pool_and_release_independent_owner_kinds() {
    let pool = BudgetPool::new(usize::MAX);
    let format = FeedFormat::Rss2;
    let feed = FeedDocument {
        id: "https://example.test/feed-id".into(),
        title: "Shared public feed & title".into(),
        home_url: "https://example.test/".into(),
        self_url: "https://example.test/feed/rss".into(),
        description: "Host-prepared public syndication".into(),
        author_name: "Author".into(),
        updated: UtcTimestamp::new(2026, 10, 27, 0, 0, 0).unwrap(),
        entries: Vec::new(),
    };
    // Rendering finishes before router construction, never inside GET. This
    // reference copy and the PublicPage body copy are explicitly unpooled.
    let rendered = render_budgeted(&feed, format, 65536, pool.clone()).unwrap();
    let rendering_charge = rendered.charged_bytes();
    let expected = rendered.as_bytes().to_vec();
    let public_site = PreparedSite::new([(
        "/feed/rss".into(),
        PublicPage::new(
            200,
            format.content_type().into(),
            rendered.as_bytes().to_vec(),
        )
        .unwrap(),
    )])
    .unwrap();
    let (app, control) =
        public_router_with_budgeted_cache(Some(public_site), "example.test", pool.clone()).unwrap();
    enable(&control);
    pool.set_limit(rendering_charge).unwrap();
    let fallback = app
        .clone()
        .oneshot(request("GET", "/feed/rss"))
        .await
        .unwrap();
    assert_eq!(fallback.status(), StatusCode::OK);
    assert_eq!(
        fallback.headers()[header::CONTENT_TYPE],
        format.content_type()
    );
    assert_eq!(
        to_bytes(fallback.into_body(), 65536)
            .await
            .unwrap()
            .as_ref(),
        expected
    );
    assert_eq!(control.status().unwrap().entries, 0);
    assert_eq!(claimed(&pool), 0);
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        rendering_charge
    );
    assert_eq!(pool.stats().unwrap().claimed, rendering_charge);
    pool.set_limit(rendering_charge + 65536).unwrap();
    let response = app
        .clone()
        .oneshot(request("GET", "/feed/rss"))
        .await
        .unwrap();
    let cache_charge = claimed(&pool);
    assert!(cache_charge > expected.len());
    assert_eq!(control.status().unwrap().entries, 1);
    assert_eq!(
        pool.stats().unwrap().claimed,
        rendering_charge + cache_charge
    );
    let body = response.into_body();
    control.purge();
    drop(app);
    drop(control);
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        rendering_charge
    );
    assert_eq!(claimed(&pool), cache_charge);
    assert_eq!(
        pool.stats().unwrap().claimed,
        rendering_charge + cache_charge
    );
    drop(rendered);
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        0
    );
    assert_eq!(claimed(&pool), cache_charge);
    assert_eq!(pool.stats().unwrap().claimed, cache_charge);
    let bytes = to_bytes(body, 65536).await.unwrap();
    assert_eq!(bytes.as_ref(), expected);
    let retained = bytes.clone();
    drop(bytes);
    assert_eq!(claimed(&pool), cache_charge);
    drop(retained);
    assert_eq!(claimed(&pool), 0);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

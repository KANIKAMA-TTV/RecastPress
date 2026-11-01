use axum::{
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
};
use recastpress_core::{
    publication::{OriginFormLocation, PreparedSite, PublicPage, RasterImageKind, RedirectStatus},
    resource_budget::{BudgetPool, ResourceKind},
};
use recastpress_server::{public_router, public_router_with_budgeted_cache};
use tower::ServiceExt;

fn text(status: u16, body: &[u8]) -> PublicPage {
    PublicPage::new(status, "text/plain; charset=utf-8".into(), body.to_vec()).unwrap()
}

fn site() -> PreparedSite {
    let mut pages = vec![
        ("/a".into(), text(200, b"0123456789")),
        ("/b".into(), text(200, b"0123456789")),
        ("/c".into(), text(200, b"0123456789")),
        ("/empty".into(), text(200, b"")),
        ("/404".into(), text(404, b"missing")),
        ("/410".into(), text(410, b"gone")),
        ("/utf8".into(), text(200, "あいう".as_bytes())),
    ];
    // A signature+all-byte fixture, not a valid image or decoder test.
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(0..=255);
    pages.push((
        "/png".into(),
        PublicPage::new_image(200, RasterImageKind::Png, png).unwrap(),
    ));
    for status in [301, 302, 303, 307, 308] {
        pages.push((
            format!("/r{status}"),
            PublicPage::new_redirect(
                RedirectStatus::try_from(status).unwrap(),
                OriginFormLocation::new("/a".into()).unwrap(),
            ),
        ));
    }
    PreparedSite::new(pages).unwrap()
}

fn app() -> axum::Router {
    public_router(Some(site()), "example.test").unwrap()
}

fn request(method: &str, target: &str, range: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(target)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap();
    if let Some(range) = range {
        request
            .headers_mut()
            .insert(header::RANGE, range.parse().unwrap());
    }
    request
}

fn protected(response: &axum::response::Response) {
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    assert!(!response.headers().contains_key(header::ACCEPT_RANGES));
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
}

#[tokio::test]
async fn single_ranges_are_inclusive_and_open_suffix_or_clamped_ranges_keep_representation_metadata()
 {
    let app = app();
    for (range, start, end) in [
        ("bytes=0-0", 0, 1),
        ("bytes=2-5", 2, 6),
        ("bytes=7-", 7, 10),
        ("bytes=-3", 7, 10),
        ("bytes=8-100", 8, 10),
        ("bytes=-100", 0, 10),
        ("bytes=0-9", 0, 10),
        ("Bytes=0002-0005", 2, 6),
        ("BYTES=-0001", 9, 10),
        (" \tbytes=1-2\t ", 1, 3),
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", "/a", Some(range)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT, "{range}");
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes {start}-{}/10", end - 1)
        );
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            (end - start).to_string()
        );
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        protected(&response);
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            &b"0123456789"[start..end]
        );
    }
}

#[tokio::test]
async fn huge_decimal_ranges_do_not_wrap_or_hide_descending_order_and_the_wire_limit_is_exact() {
    let app = app();
    let huge = "9".repeat(100);
    let lower = format!("1{}", "0".repeat(99));
    for range in [format!("bytes=0-{huge}"), format!("bytes=-{huge}")] {
        let response = app
            .clone()
            .oneshot(request("GET", "/a", Some(&range)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 0-9/10");
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            b"0123456789"
        );
    }
    let response = app
        .clone()
        .oneshot(request("GET", "/a", Some(&format!("bytes={huge}-{huge}"))))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    let descending = format!("bytes={huge}-{lower}");
    let response = app
        .clone()
        .oneshot(request("GET", "/a", Some(&descending)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::CONTENT_RANGE));
    let boundary = format!("bytes={}1-2", "0".repeat(247));
    assert_eq!(boundary.len(), 256);
    let response = app
        .clone()
        .oneshot(request("GET", "/a", Some(&boundary)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
        b"12"
    );
    let over = format!("{boundary} ");
    assert_eq!(over.len(), 257);
    let response = app
        .oneshot(request("GET", "/a", Some(&over)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::CONTENT_RANGE));
    assert_eq!(
        to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
        b"0123456789"
    );
}

#[tokio::test]
async fn malformed_or_multiple_ranges_are_full_but_valid_unsatisfiable_ranges_are_empty_416() {
    let app = app();
    for range in [
        "bytes=",
        "bytes=-",
        "bytes=2-1",
        "bytes=+1-2",
        "bytes=1.0-2",
        "bytes=1e0-2",
        "bytes=1 -2",
        "bytes= 1-2",
        "bytes=1- 2",
        "bytes=0-1,3-4",
        "bytes=0-1,999-",
        "items=0-1",
        "bytes =0-1",
        "bytes=--1",
        "bytes=0-1junk",
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", "/a", Some(range)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{range}");
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        protected(&response);
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            b"0123456789"
        );
    }
    let mut duplicate = request("GET", "/a", Some("bytes=0-1"));
    duplicate
        .headers_mut()
        .append(header::RANGE, "bytes=2-3".parse().unwrap());
    let mut nonascii = request("GET", "/a", None);
    nonascii.headers_mut().insert(
        header::RANGE,
        HeaderValue::from_bytes(b"bytes=0-\xff").unwrap(),
    );
    for request in [duplicate, nonascii] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            b"0123456789"
        );
    }
    for range in [
        "bytes=10-",
        "bytes=10-20",
        "bytes=100-100",
        "bytes=-0",
        "bytes=-000",
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", "/a", Some(range)))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "{range}"
        );
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */10");
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        protected(&response);
        assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
    }
    for range in [
        "bytes=0-0",
        "bytes=1-",
        "bytes=-0",
        "bytes=-1",
        "bytes=-999",
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", "/empty", Some(range)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn byte_offsets_split_utf8_and_binary_without_decoding_or_changing_mime() {
    let app = app();
    let response = app
        .clone()
        .oneshot(request("GET", "/utf8", Some("bytes=1-2")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 1-2/9");
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let bytes = to_bytes(response.into_body(), 2).await.unwrap();
    assert_eq!(bytes.as_ref(), &"あいう".as_bytes()[1..3]);
    assert!(std::str::from_utf8(&bytes).is_err());
    let response = app
        .oneshot(request("GET", "/png", Some("bytes=8-263")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 8-263/264");
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "256");
    protected(&response);
    assert_eq!(
        to_bytes(response.into_body(), 256).await.unwrap().as_ref(),
        &(0..=255).collect::<Vec<u8>>()
    );
}

#[tokio::test]
async fn head_non200_health_and_evaluated_conditions_keep_their_selected_response() {
    let app = app();
    for (target, status, body) in [
        ("/404", 404, b"missing".as_slice()),
        ("/410", 410, b"gone".as_slice()),
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", target, Some("bytes=0-0")))
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(
            to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
            body
        );
    }
    for status in [301, 302, 303, 307, 308] {
        for method in ["GET", "HEAD"] {
            let response = app
                .clone()
                .oneshot(request(method, &format!("/r{status}"), Some("bytes=0-0")))
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
            assert_eq!(response.headers()[header::LOCATION], "/a");
            assert!(!response.headers().contains_key(header::CONTENT_RANGE));
            assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
        }
    }
    for range in ["bytes=1-2", "bytes=999-", "bytes=0-0,2-3"] {
        let response = app
            .clone()
            .oneshot(request("HEAD", "/a", Some(range)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "10");
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
    }
    for (target, body) in [
        ("/health/live", b"live\n".as_slice()),
        ("/health/ready", b"ready\n".as_slice()),
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", target, Some("bytes=0-0")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(
            to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
            body
        );
    }
    // The ETag phase supersedes the earlier any-conditional => Full profile.
    // Dates and nonmatching If-Range still suppress Range; evaluated tag lists
    // can instead produce 206/304/412/400 in their explicit priority order.
    for name in [
        header::IF_RANGE,
        header::IF_MATCH,
        header::IF_NONE_MATCH,
        header::IF_MODIFIED_SINCE,
        header::IF_UNMODIFIED_SINCE,
    ] {
        for value in ["", "*", "\"validator\"", "invalid date"] {
            let expected = match (name.as_str(), value) {
                ("if-match", "*") | ("if-none-match", "" | "\"validator\"") => {
                    StatusCode::PARTIAL_CONTENT
                }
                ("if-match", "" | "\"validator\"") => StatusCode::PRECONDITION_FAILED,
                ("if-none-match", "*") => StatusCode::NOT_MODIFIED,
                ("if-match" | "if-none-match", _) => StatusCode::BAD_REQUEST,
                _ => StatusCode::OK,
            };
            let mut req = request("GET", "/a", Some("bytes=1-2"));
            req.headers_mut()
                .insert(name.clone(), value.parse().unwrap());
            let response = app.clone().oneshot(req).await.unwrap();
            assert_eq!(response.status(), expected, "{name}={value:?}");
            if expected == StatusCode::PARTIAL_CONTENT {
                assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 1-2/10");
            } else {
                assert!(!response.headers().contains_key(header::CONTENT_RANGE));
            }
            let expected_body = match expected {
                StatusCode::OK => b"0123456789".as_slice(),
                StatusCode::PARTIAL_CONTENT => b"12".as_slice(),
                _ => b"".as_slice(),
            };
            let expected_length = if expected == StatusCode::NOT_MODIFIED {
                10
            } else {
                expected_body.len()
            };
            assert_eq!(
                response.headers()[header::CONTENT_LENGTH],
                expected_length.to_string()
            );
            assert_eq!(
                to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
                expected_body
            );
        }
    }
    let mut wrong = request("GET", "/a", Some("bytes=0-0"));
    wrong
        .headers_mut()
        .insert(header::HOST, "evil.test".parse().unwrap());
    for (request, status) in [
        (wrong, StatusCode::MISDIRECTED_REQUEST),
        (
            request("POST", "/a", Some("bytes=0-0")),
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        (
            request("GET", "/not-listed", Some("bytes=0-0")),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), status);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
    }
}

#[tokio::test]
async fn range_shared_payloads_leave_cache_owners_lru_and_zero_pool_unchanged() {
    for capacity in [0, 65536] {
        let pool = BudgetPool::new(capacity);
        let (app, control) =
            public_router_with_budgeted_cache(Some(site()), "example.test", pool.clone()).unwrap();
        assert!(control.apply_allocation(65536));
        assert!(control.reconciliation_completed());
        let first = app
            .clone()
            .oneshot(request("GET", "/a", None))
            .await
            .unwrap();
        let owner = to_bytes(first.into_body(), 10).await.unwrap();
        let before = control.status().unwrap();
        let budget = pool.stats().unwrap();
        assert_eq!(before.entries, usize::from(capacity != 0));
        for (range, expected) in [
            ("bytes=1-2", StatusCode::PARTIAL_CONTENT),
            ("bytes=10-", StatusCode::RANGE_NOT_SATISFIABLE),
            ("bytes=2-1", StatusCode::OK),
            ("bytes=0-0,2-2", StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(request("GET", "/a", Some(range)))
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            let partial = expected == StatusCode::PARTIAL_CONTENT;
            let body = to_bytes(response.into_body(), 10).await.unwrap();
            if partial {
                assert_eq!(body.as_ref(), b"12");
                if capacity == 0 {
                    // Both responses share the original sealed page. The first
                    // full response could not obtain a cache lease either.
                    assert_eq!(body.as_ptr(), owner.as_ptr().wrapping_add(1));
                } else {
                    // A successful cache admission still owns its own copy.
                    assert_ne!(body.as_ptr(), owner.as_ptr().wrapping_add(1));
                }
            }
            assert_eq!(control.status().unwrap(), before);
            assert_eq!(pool.stats().unwrap(), budget);
        }
        let mut conditional = request("GET", "/a", Some("bytes=1-2"));
        conditional
            .headers_mut()
            .insert(header::IF_MATCH, "*".parse().unwrap());
        let response = app.clone().oneshot(conditional).await.unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            b"12"
        );
        assert_eq!(control.status().unwrap(), before);
        assert_eq!(pool.stats().unwrap(), budget);
        let repeated = app
            .clone()
            .oneshot(request("GET", "/a", None))
            .await
            .unwrap();
        let repeated = to_bytes(repeated.into_body(), 10).await.unwrap();
        assert_eq!(repeated.as_ref(), b"0123456789");
        assert_eq!(repeated.as_ptr(), owner.as_ptr());
        drop(repeated);
        if capacity != 0 {
            let charge = budget.claimed_for(ResourceKind::ResponseCache);
            assert!(charge > 10);
            assert!(control.apply_allocation(charge * 2));
            // A local allocation shrink intentionally purges the old epoch.
            // Start the two-entry LRU experiment with fresh owners afterward.
            drop(owner);
            let fresh = app
                .clone()
                .oneshot(request("GET", "/a", None))
                .await
                .unwrap();
            let owner = to_bytes(fresh.into_body(), 10).await.unwrap();
            drop(
                app.clone()
                    .oneshot(request("GET", "/b", None))
                    .await
                    .unwrap(),
            );
            assert_eq!(control.status().unwrap().entries, 2);
            let before = control.status().unwrap();
            let budget = pool.stats().unwrap();
            drop(
                app.clone()
                    .oneshot(request("GET", "/a", Some("bytes=0-0")))
                    .await
                    .unwrap(),
            );
            assert_eq!(control.status().unwrap(), before);
            assert_eq!(pool.stats().unwrap(), budget);
            drop(app.oneshot(request("GET", "/c", None)).await.unwrap());
            // A was oldest; a Range A must not promote it. Its retained Bytes
            // owner keeps a third lease after C evicts A from the two-entry LRU.
            assert_eq!(control.status().unwrap().entries, 2);
            assert_eq!(pool.stats().unwrap().claimed, charge * 3);
            drop(owner);
            assert_eq!(pool.stats().unwrap().claimed, charge * 2);
        } else {
            drop(owner);
            assert_eq!(pool.stats().unwrap().claimed, 0);
        }
        assert!(control.purge());
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

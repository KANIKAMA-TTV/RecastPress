use axum::{
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
};
use recastpress_core::{
    publication::{OriginFormLocation, PreparedSite, PublicPage, RedirectStatus},
    resource_budget::BudgetPool,
};
use recastpress_server::{public_router, public_router_with_budgeted_cache};
use tower::ServiceExt;

fn site() -> PreparedSite {
    let mut pages = vec![];
    for (target, status, mime, bytes) in [
        (
            "/a",
            200,
            "text/plain; charset=utf-8",
            b"0123456789".as_slice(),
        ),
        (
            "/b",
            200,
            "text/plain; charset=utf-8",
            b"0123456789".as_slice(),
        ),
        (
            "/c",
            200,
            "text/plain; charset=utf-8",
            b"0123456789".as_slice(),
        ),
        (
            "/mime",
            200,
            "text/html; charset=utf-8",
            b"0123456789".as_slice(),
        ),
        (
            "/different",
            200,
            "text/plain; charset=utf-8",
            b"012345678X".as_slice(),
        ),
        ("/empty", 200, "text/plain; charset=utf-8", b"".as_slice()),
        (
            "/404",
            404,
            "text/plain; charset=utf-8",
            b"missing".as_slice(),
        ),
        ("/410", 410, "text/plain; charset=utf-8", b"gone".as_slice()),
    ] {
        pages.push((
            target.into(),
            PublicPage::new(status, mime.into(), bytes.to_vec()).unwrap(),
        ));
    }
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
fn request(method: &str, target: &str, fields: &[(&str, &[u8])]) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(target)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap();
    for (name, value) in fields {
        req.headers_mut().append(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_bytes(value).unwrap(),
        );
    }
    req
}
async fn tag(app: &axum::Router, target: &str) -> String {
    let response = app
        .clone()
        .oneshot(request("GET", target, &[]))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned()
}
fn protected(response: &axum::response::Response) {
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
}
async fn condition_response(
    app: &axum::Router,
    method: &str,
    fields: &[(&str, &[u8])],
    expected: StatusCode,
    etag: &str,
) {
    let response = app
        .clone()
        .oneshot(request(method, "/a", fields))
        .await
        .unwrap();
    assert_eq!(response.status(), expected);
    protected(&response);
    assert!(!response.headers().contains_key(header::CONTENT_RANGE));
    if expected == StatusCode::OK || expected == StatusCode::NOT_MODIFIED {
        assert_eq!(response.headers()[header::ETAG], etag);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "10");
    } else {
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        assert!(!response.headers().contains_key(header::ETAG));
    }
    if expected == StatusCode::NOT_MODIFIED {
        assert!(!response.headers().contains_key(header::CONTENT_TYPE));
    }
    let body = to_bytes(response.into_body(), 10).await.unwrap();
    assert_eq!(
        body.as_ref(),
        if method == "GET" && expected == StatusCode::OK {
            b"0123456789".as_slice()
        } else {
            b"".as_slice()
        }
    );
}

#[tokio::test]
async fn startup_tags_are_strong_stable_for_identical_representations_and_cover_get_head_empty_and_ranges()
 {
    let app = app();
    let first = tag(&app, "/a").await;
    assert_eq!(first.len(), 66);
    assert!(first.starts_with('"') && first.ends_with('"'));
    assert!(
        first.as_bytes()[1..65]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    );
    assert_eq!(tag(&app, "/b").await, first);
    assert_ne!(tag(&app, "/mime").await, first);
    assert_ne!(tag(&app, "/different").await, first);
    assert_eq!(
        tag(&public_router(Some(site()), "example.test").unwrap(), "/a").await,
        first
    );
    for target in ["/a", "/empty"] {
        let expected = tag(&app, target).await;
        let length = if target == "/empty" { "0" } else { "10" };
        let head = app
            .clone()
            .oneshot(request("HEAD", target, &[]))
            .await
            .unwrap();
        assert_eq!(head.headers()[header::ETAG], expected);
        assert_eq!(head.headers()[header::CONTENT_LENGTH], length);
        assert!(to_bytes(head.into_body(), 0).await.unwrap().is_empty());
        let unchanged = app
            .clone()
            .oneshot(request(
                "GET",
                target,
                &[("if-none-match", expected.as_bytes())],
            ))
            .await
            .unwrap();
        assert_eq!(unchanged.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(unchanged.headers()[header::CONTENT_LENGTH], length);
        assert_eq!(unchanged.headers()[header::ETAG], expected);
        assert!(to_bytes(unchanged.into_body(), 0).await.unwrap().is_empty());
    }
    let partial = app
        .oneshot(request("GET", "/a", &[("range", b"bytes=1-2")]))
        .await
        .unwrap();
    assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(partial.headers()[header::ETAG], first);
    assert_eq!(
        to_bytes(partial.into_body(), 2).await.unwrap().as_ref(),
        b"12"
    );
}

#[tokio::test]
async fn strong_if_match_and_weak_if_none_match_lists_and_wildcards_apply_to_get_and_head() {
    let app = app();
    let current = tag(&app, "/a").await;
    let weak = format!("W/{current}");
    let matching = format!("\"other\", {current}");
    let weak_matching = format!("\"other\", {weak}");
    for method in ["GET", "HEAD"] {
        for (name, value, status) in [
            ("if-match", current.as_str(), StatusCode::OK),
            ("if-match", weak.as_str(), StatusCode::PRECONDITION_FAILED),
            ("if-match", matching.as_str(), StatusCode::OK),
            ("if-match", "*", StatusCode::OK),
            ("if-match", "", StatusCode::PRECONDITION_FAILED),
            ("if-none-match", current.as_str(), StatusCode::NOT_MODIFIED),
            ("if-none-match", weak.as_str(), StatusCode::NOT_MODIFIED),
            (
                "if-none-match",
                weak_matching.as_str(),
                StatusCode::NOT_MODIFIED,
            ),
            ("if-none-match", "*", StatusCode::NOT_MODIFIED),
            ("if-none-match", "", StatusCode::OK),
            ("if-none-match", "\"other\"", StatusCode::OK),
        ] {
            condition_response(&app, method, &[(name, value.as_bytes())], status, &current).await;
        }
    }
}

#[tokio::test]
async fn complete_raw_lists_preserve_repeated_fields_and_reject_malformed_or_oversize_late_members()
{
    let app = app();
    let current = tag(&app, "/a").await;
    let weak = format!("W/{current}");
    condition_response(
        &app,
        "GET",
        &[
            ("if-none-match", b"\"other,comma\\literal\""),
            ("if-none-match", weak.as_bytes()),
        ],
        StatusCode::NOT_MODIFIED,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("if-match", b"\"other\""), ("if-match", current.as_bytes())],
        StatusCode::OK,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("if-none-match", b"\"\xff\"")],
        StatusCode::OK,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("if-none-match", format!(",, {weak},,").as_bytes())],
        StatusCode::NOT_MODIFIED,
        &current,
    )
    .await;
    for bad in [
        format!("{current},bad"),
        format!("{current}junk"),
        format!("{current},*"),
        "w/\"other\"".into(),
        "W/ \"other\"".into(),
        "\"space inside\"".into(),
        "\"unterminated".into(),
        "*,".into(),
    ] {
        for name in ["if-match", "if-none-match"] {
            condition_response(
                &app,
                "GET",
                &[(name, bad.as_bytes())],
                StatusCode::BAD_REQUEST,
                &current,
            )
            .await;
        }
    }
    condition_response(
        &app,
        "GET",
        &[("if-match", b"*"), ("if-match", current.as_bytes())],
        StatusCode::BAD_REQUEST,
        &current,
    )
    .await;
    let exact = format!("\"{}\"", "x".repeat(4094));
    assert_eq!(exact.len(), 4096);
    condition_response(
        &app,
        "GET",
        &[("if-none-match", exact.as_bytes())],
        StatusCode::OK,
        &current,
    )
    .await;
    let over = format!("{exact} ");
    condition_response(
        &app,
        "GET",
        &[("if-none-match", over.as_bytes())],
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
        &current,
    )
    .await;
    let left = format!("\"{}\"", "x".repeat(2045));
    let right = format!("\"{}\"", "y".repeat(2046));
    assert_eq!(left.len() + right.len() + 1, 4096);
    condition_response(
        &app,
        "GET",
        &[
            ("if-none-match", left.as_bytes()),
            ("if-none-match", right.as_bytes()),
        ],
        StatusCode::OK,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[
            ("if-none-match", left.as_bytes()),
            ("if-none-match", format!("{right} ").as_bytes()),
        ],
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
        &current,
    )
    .await;
    let sixty_four = format!("{},{}", vec!["\"other\""; 63].join(","), weak);
    condition_response(
        &app,
        "GET",
        &[("if-none-match", sixty_four.as_bytes())],
        StatusCode::NOT_MODIFIED,
        &current,
    )
    .await;
    let sixty_five = format!("{current},{}", vec!["\"other\""; 64].join(","));
    condition_response(
        &app,
        "GET",
        &[("if-none-match", sixty_five.as_bytes())],
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("if-match", ",".repeat(63).as_bytes())],
        StatusCode::PRECONDITION_FAILED,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("if-match", ",".repeat(64).as_bytes())],
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
        &current,
    )
    .await;
}

#[tokio::test]
async fn precondition_priority_and_if_range_allow_only_an_exact_strong_validator_after_tag_checks()
{
    let app = app();
    let current = tag(&app, "/a").await;
    let weak = format!("W/{current}");
    let huge = vec![b'x'; 4097];
    condition_response(
        &app,
        "GET",
        &[
            ("if-match", b"\"other\""),
            ("if-none-match", current.as_bytes()),
            ("range", b"bytes=1-2"),
        ],
        StatusCode::PRECONDITION_FAILED,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[
            ("if-none-match", current.as_bytes()),
            ("range", b"bytes=999-"),
            ("if-range", &huge),
        ],
        StatusCode::NOT_MODIFIED,
        &current,
    )
    .await;
    for (range, status, expected) in [
        (
            b"bytes=1-2".as_slice(),
            StatusCode::PARTIAL_CONTENT,
            b"12".as_slice(),
        ),
        (
            b"bytes=999-".as_slice(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            b"".as_slice(),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/a",
                &[
                    ("if-match", current.as_bytes()),
                    ("if-none-match", b"\"other\""),
                    ("range", range),
                    ("if-range", current.as_bytes()),
                ],
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert!(response.headers().contains_key(header::CONTENT_RANGE));
        protected(&response);
        if status == StatusCode::PARTIAL_CONTENT {
            assert_eq!(response.headers()[header::ETAG], current);
            assert_eq!(response.headers()[header::CONTENT_LENGTH], "2");
        }
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            expected
        );
    }
    for value in [
        weak.as_bytes(),
        b"\"other\"",
        b"Sat, 29 Oct 1994 19:43:31 GMT",
        b"bad",
        b"*",
    ] {
        condition_response(
            &app,
            "GET",
            &[("range", b"bytes=1-2"), ("if-range", value)],
            StatusCode::OK,
            &current,
        )
        .await;
    }
    condition_response(
        &app,
        "GET",
        &[
            ("range", b"bytes=1-2"),
            ("if-range", current.as_bytes()),
            ("if-range", current.as_bytes()),
        ],
        StatusCode::OK,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("range", b"bytes=1-2"), ("if-range", &huge)],
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
        &current,
    )
    .await;
    condition_response(
        &app,
        "HEAD",
        &[("range", b"bytes=1-2"), ("if-range", &huge)],
        StatusCode::OK,
        &current,
    )
    .await;
    condition_response(
        &app,
        "GET",
        &[("if-range", &huge)],
        StatusCode::OK,
        &current,
    )
    .await;
    for date in ["if-modified-since", "if-unmodified-since"] {
        condition_response(
            &app,
            "GET",
            &[(date, b""), ("if-match", b"\"other\"")],
            StatusCode::PRECONDITION_FAILED,
            &current,
        )
        .await;
        condition_response(
            &app,
            "GET",
            &[
                (date, b"bad date"),
                ("if-none-match", weak.as_bytes()),
                ("range", b"bytes=1-2"),
            ],
            StatusCode::NOT_MODIFIED,
            &current,
        )
        .await;
        condition_response(
            &app,
            "GET",
            &[
                (date, b"bad date"),
                ("if-match", current.as_bytes()),
                ("range", b"bytes=1-2"),
                ("if-range", current.as_bytes()),
            ],
            StatusCode::OK,
            &current,
        )
        .await;
    }
}

#[tokio::test]
async fn non200_routes_and_normal_request_rejections_precede_conditions_without_etag_leakage() {
    let app = app();
    let huge = vec![b'x'; 4097];
    let fields = [
        ("if-match", huge.as_slice()),
        ("if-none-match", b"*".as_slice()),
        ("range", b"bytes=0-0".as_slice()),
    ];
    for (target, status) in [
        ("/404", 404),
        ("/410", 410),
        ("/not-listed", 404),
        ("/r301", 301),
        ("/r302", 302),
        ("/r303", 303),
        ("/r307", 307),
        ("/r308", 308),
        ("/health/live", 200),
        ("/health/ready", 200),
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", target, &fields))
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert!(!response.headers().contains_key(header::ETAG));
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        protected(&response);
        if (300..400).contains(&status) {
            assert_eq!(response.headers()[header::LOCATION], "/a");
            assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
        }
    }
    let response = app
        .clone()
        .oneshot(request("POST", "/a", &fields))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(!response.headers().contains_key(header::ETAG));
    let mut wrong = request("GET", "/a", &fields);
    wrong
        .headers_mut()
        .insert(header::HOST, "evil.test".parse().unwrap());
    let response = app.oneshot(wrong).await.unwrap();
    assert_eq!(response.status(), StatusCode::MISDIRECTED_REQUEST);
    assert!(!response.headers().contains_key(header::ETAG));
}

#[tokio::test]
async fn conditional_shared_bodies_preserve_cache_stats_lru_and_zero_pool() {
    for capacity in [0, 65536] {
        let pool = BudgetPool::new(capacity);
        let (app, control) =
            public_router_with_budgeted_cache(Some(site()), "example.test", pool.clone()).unwrap();
        assert!(control.apply_allocation(65536));
        assert!(control.reconciliation_completed());
        let response = app
            .clone()
            .oneshot(request("GET", "/a", &[]))
            .await
            .unwrap();
        let current = response.headers()[header::ETAG]
            .to_str()
            .unwrap()
            .to_owned();
        let owner = to_bytes(response.into_body(), 10).await.unwrap();
        let before = control.status().unwrap();
        let budget = pool.stats().unwrap();
        let late_bad = format!("{current},bad");
        let huge = vec![b'x'; 4097];
        for (fields, status) in [
            (
                vec![("if-none-match", current.as_bytes())],
                StatusCode::NOT_MODIFIED,
            ),
            (
                vec![("if-match", b"\"other\"".as_slice())],
                StatusCode::PRECONDITION_FAILED,
            ),
            (
                vec![("if-match", late_bad.as_bytes())],
                StatusCode::BAD_REQUEST,
            ),
            (
                vec![("if-match", huge.as_slice())],
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            ),
            (vec![("if-match", current.as_bytes())], StatusCode::OK),
            (
                vec![
                    ("if-match", current.as_bytes()),
                    ("range", b"bytes=1-2".as_slice()),
                ],
                StatusCode::PARTIAL_CONTENT,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(request("GET", "/a", &fields))
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(control.status().unwrap(), before);
            assert_eq!(pool.stats().unwrap(), budget);
            let bytes = to_bytes(response.into_body(), 10).await.unwrap();
            if status == StatusCode::OK {
                assert_eq!(bytes.as_ref(), owner.as_ref());
                if capacity == 0 {
                    assert_eq!(bytes.as_ptr(), owner.as_ptr());
                } else {
                    assert_ne!(bytes.as_ptr(), owner.as_ptr());
                }
            }
            if status == StatusCode::PARTIAL_CONTENT {
                assert_eq!(bytes.as_ref(), b"12");
                if capacity == 0 {
                    assert_eq!(bytes.as_ptr(), owner.as_ptr().wrapping_add(1));
                } else {
                    assert_ne!(bytes.as_ptr(), owner.as_ptr().wrapping_add(1));
                }
            }
            assert_eq!(control.status().unwrap(), before);
            assert_eq!(pool.stats().unwrap(), budget);
        }
        let response = app
            .clone()
            .oneshot(request("GET", "/a", &[]))
            .await
            .unwrap();
        let again = to_bytes(response.into_body(), 10).await.unwrap();
        assert_eq!(again.as_ptr(), owner.as_ptr());
        drop(again);
        if capacity != 0 {
            let charge = budget.claimed;
            assert!(charge > 10);
            assert!(control.apply_allocation(charge * 2));
            drop(owner);
            let response = app
                .clone()
                .oneshot(request("GET", "/a", &[]))
                .await
                .unwrap();
            let owner = to_bytes(response.into_body(), 10).await.unwrap();
            drop(
                app.clone()
                    .oneshot(request("GET", "/b", &[]))
                    .await
                    .unwrap(),
            );
            let before = control.status().unwrap();
            let budget = pool.stats().unwrap();
            assert_eq!(before.entries, 2);
            condition_response(
                &app,
                "GET",
                &[("if-none-match", b"\"other\"")],
                StatusCode::OK,
                &current,
            )
            .await;
            condition_response(
                &app,
                "GET",
                &[("if-none-match", current.as_bytes())],
                StatusCode::NOT_MODIFIED,
                &current,
            )
            .await;
            assert_eq!(control.status().unwrap(), before);
            assert_eq!(pool.stats().unwrap(), budget);
            drop(app.oneshot(request("GET", "/c", &[])).await.unwrap());
            assert_eq!(control.status().unwrap().entries, 2);
            // If either conditional A had touched LRU, B would be evicted and
            // only two leases would remain. Retained A proves the intended third.
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

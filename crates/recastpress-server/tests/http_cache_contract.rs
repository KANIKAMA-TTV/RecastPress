use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use recastpress_core::{
    cache::{MonitorState, StopReason},
    publication::{PreparedSite, PublicPage},
};
use recastpress_server::{http_cache::PublicCacheControl, public_router_with_cache};
use tower::ServiceExt;

fn app(targets: &[(&str, u16)]) -> (axum::Router, PublicCacheControl) {
    let site = PreparedSite::new(targets.iter().map(|(target, status)| {
        (
            (*target).into(),
            PublicPage::new(
                *status,
                "text/plain; charset=utf-8".into(),
                target.as_bytes().to_vec(),
            )
            .unwrap(),
        )
    }))
    .unwrap();
    public_router_with_cache(Some(site), "example.test").unwrap()
}

fn request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap()
}

fn enable(control: &PublicCacheControl) {
    assert!(control.apply_allocation(64 * 1024));
    assert!(control.reconciliation_completed());
}

#[tokio::test]
async fn suspended_cache_serves_public_output_and_needs_both_prerequisites() {
    let (app, control) = app(&[("/", 200)]);
    assert_eq!(
        control.status().unwrap().stop_reason,
        Some(StopReason::MonitorNotReady)
    );
    for step in 0..3 {
        if step == 1 {
            control.reconciliation_completed();
            assert_eq!(
                control.status().unwrap().stop_reason,
                Some(StopReason::NoBudget)
            );
        } else if step == 2 {
            control.apply_allocation(4096);
        }
        let response = app.clone().oneshot(request("GET", "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(to_bytes(response.into_body(), 100).await.unwrap(), "/");
        assert_eq!(control.status().unwrap().entries, usize::from(step == 2));
    }
}

#[tokio::test]
async fn exact_public_query_variants_are_distinct_and_all_purged() {
    let targets = [
        "/",
        "/?s=a&page=1",
        "/?page=1&s=a",
        "/?s=a&s=b",
        "/?s=a+b",
        "/?s=a%20b",
    ];
    let rows: Vec<_> = targets.iter().map(|target| (*target, 200)).collect();
    let (app, control) = app(&rows);
    enable(&control);
    for _ in 0..2 {
        for target in targets {
            let response = app.clone().oneshot(request("GET", target)).await.unwrap();
            assert_eq!(to_bytes(response.into_body(), 100).await.unwrap(), target);
        }
        assert_eq!(control.status().unwrap().entries, targets.len());
    }
    assert!(control.purge());
    assert_eq!(control.status().unwrap().entries, 0);
}

#[tokio::test]
async fn credentials_request_controls_and_unknown_headers_bypass_shared_storage() {
    let (app, control) = app(&[("/", 200)]);
    enable(&control);
    for name in [
        "authorization",
        "proxy-authorization",
        "cookie",
        "cache-control",
        "pragma",
        "range",
        "if-range",
        "if-match",
        "if-none-match",
        "if-modified-since",
        "if-unmodified-since",
        "content-length",
        "transfer-encoding",
        "content-type",
        "x-wp-nonce",
        "x-custom-identity",
        "forwarded",
        "x-forwarded-host",
    ] {
        let mut req = request("GET", "/");
        // Exercise cache bypass AFTER a valid condition has passed. Malformed
        // entity-tag fields now receive 400 before body/cache processing.
        let value = match name {
            "if-match" => "*",
            "if-none-match" => "\"foreign\"",
            _ => "test",
        };
        req.headers_mut().insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{name}");
        assert_eq!(to_bytes(response.into_body(), 100).await.unwrap(), "/");
        assert_eq!(control.status().unwrap().entries, 0, "{name}");
    }
    let mut body_request = request("GET", "/");
    *body_request.body_mut() = Body::from("private request body");
    app.clone().oneshot(body_request).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 0);
    app.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 1);
}

#[tokio::test]
async fn unknown_secret_or_encoded_query_names_bypass_without_changing_routes() {
    let targets = [
        "/?code=secret",
        "/?s=public&token=secret",
        "/?preview=true",
        "/?%73=secret",
        "/?S=secret",
        "/?s=a;token=secret",
        "/?s=a&",
        "/?",
    ];
    let rows: Vec<_> = targets.iter().map(|target| (*target, 200)).collect();
    let (app, control) = app(&rows);
    enable(&control);
    for target in targets {
        let response = app.clone().oneshot(request("GET", target)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{target}");
        assert_eq!(to_bytes(response.into_body(), 100).await.unwrap(), target);
        assert_eq!(control.status().unwrap().entries, 0, "{target}");
    }
}

#[tokio::test]
async fn head_errors_health_and_wrong_host_never_fill_cache() {
    let (app, control) = app(&[("/", 200), ("/gone", 410), ("/custom-missing", 404)]);
    enable(&control);
    for (method, target) in [
        ("HEAD", "/"),
        ("GET", "/missing"),
        ("GET", "/gone"),
        ("GET", "/custom-missing"),
        ("GET", "/health/live"),
        ("GET", "/health/ready"),
        ("POST", "/"),
    ] {
        app.clone().oneshot(request(method, target)).await.unwrap();
        assert_eq!(control.status().unwrap().entries, 0);
    }
    let mut req = request("GET", "/");
    req.headers_mut()
        .insert(header::HOST, "evil.test".parse().unwrap());
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::MISDIRECTED_REQUEST
    );
    assert_eq!(control.status().unwrap().entries, 0);
}

#[tokio::test]
async fn loss_recovery_disable_and_budget_shrink_release_entries() {
    let (app, control) = app(&[("/", 200)]);
    enable(&control);
    app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 1);
    control.monitoring_lost();
    assert!(control.status().unwrap().configured_enabled);
    assert_eq!(control.status().unwrap().monitor, MonitorState::Unavailable);
    app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 0);
    control.begin_reconciliation();
    app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 0);
    control.reconciliation_completed();
    app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 1);
    control.set_enabled(false);
    app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 0);
    control.set_enabled(true);
    control.apply_allocation(1);
    app.clone().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(control.status().unwrap().entries, 0);
    control.apply_allocation(0);
    assert_eq!(
        control.status().unwrap().stop_reason,
        Some(StopReason::NoBudget)
    );
}

#[tokio::test]
async fn separately_sealed_routers_never_share_entries_or_control() {
    let (first, first_control) = app(&[("/", 200)]);
    let (second, second_control) = app(&[("/second", 200)]);
    enable(&first_control);
    enable(&second_control);
    first.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(first_control.status().unwrap().entries, 1);
    assert_eq!(second_control.status().unwrap().entries, 0);
    let response = second.oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(second_control.status().unwrap().entries, 0);
    second_control.purge();
    assert_eq!(first_control.status().unwrap().entries, 1);
}

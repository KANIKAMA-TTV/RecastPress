use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use recastpress_core::publication::{MAX_TARGET_BYTES, PrepareError, PreparedSite, PublicPage};
use recastpress_server::{bundle, management_router, public_router};
use tower::ServiceExt;

fn app() -> axum::Router {
    let site = bundle::parse(include_bytes!("../../../examples/public-bundle.json")).unwrap();
    public_router(Some(site), "example.test").unwrap()
}

fn request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn prepared_route_and_http_target_share_the_same_size_boundary() {
    let target = format!("/?q={}", "a".repeat(MAX_TARGET_BYTES - 4));
    let page = PublicPage::new(
        200,
        "text/plain; charset=utf-8".into(),
        b"boundary".to_vec(),
    )
    .unwrap();
    let site = PreparedSite::new([(target.clone(), page.clone())]).unwrap();
    let router = public_router(Some(site), "example.test").unwrap();
    let response = router
        .clone()
        .oneshot(request("GET", &target))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap(),
        "boundary"
    );
    let oversize = format!("{target}a");
    assert_eq!(
        PreparedSite::new([(oversize.clone(), page)]).unwrap_err(),
        PrepareError::InvalidTarget
    );
    let rejected = router.oneshot(request("GET", &oversize)).await.unwrap();
    assert_eq!(rejected.status(), StatusCode::URI_TOO_LONG);
    assert_eq!(rejected.headers()[header::CACHE_CONTROL], "no-store");
}

#[tokio::test]
async fn public_routes_preserve_queries_and_headers() {
    let response = app().oneshot(request("GET", "/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    let length = response.headers()[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let body = to_bytes(response.into_body(), 10000).await.unwrap();
    assert_eq!(body.len(), length);
    assert!(std::str::from_utf8(&body).unwrap().contains("RecastPress"));
    let found = app().oneshot(request("GET", "/?s=rust")).await.unwrap();
    assert_eq!(found.status(), StatusCode::OK);
    let missing = app().oneshot(request("GET", "/?s=php")).await.unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn head_has_get_length_but_no_body_for_success_and_failure() {
    let get = app().oneshot(request("GET", "/")).await.unwrap();
    let head = app().oneshot(request("HEAD", "/")).await.unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(
        head.headers()[header::CONTENT_LENGTH],
        get.headers()[header::CONTENT_LENGTH]
    );
    assert!(to_bytes(head.into_body(), 10000).await.unwrap().is_empty());
    let missing = app().oneshot(request("HEAD", "/missing")).await.unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert!(
        to_bytes(missing.into_body(), 10000)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn wrong_missing_and_duplicate_host_are_rejected() {
    for host in [None, Some("evil.test"), Some("example.test.evil")] {
        let mut req = request("GET", "/");
        req.headers_mut().remove(header::HOST);
        if let Some(host) = host {
            req.headers_mut()
                .insert(header::HOST, host.parse().unwrap());
        }
        assert_eq!(
            app().oneshot(req).await.unwrap().status(),
            StatusCode::MISDIRECTED_REQUEST
        );
    }
    let mut req = request("GET", "/");
    req.headers_mut()
        .append(header::HOST, "example.test".parse().unwrap());
    assert_eq!(
        app().oneshot(req).await.unwrap().status(),
        StatusCode::MISDIRECTED_REQUEST
    );
    let mut req = request("GET", "/");
    req.headers_mut()
        .insert(header::HOST, "EXAMPLE.TEST".parse().unwrap());
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn public_mutation_and_proxy_targets_are_rejected() {
    let response = app().oneshot(request("POST", "/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[header::ALLOW], "GET, HEAD");
    let response = app()
        .oneshot(request("GET", "http://example.test/"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn liveness_is_independent_of_readiness_and_management_has_no_mutations() {
    for (router, ready) in [
        (app(), StatusCode::OK),
        (
            public_router(None, "example.test").unwrap(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (management_router(), StatusCode::SERVICE_UNAVAILABLE),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(request("GET", "/health/live"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            router
                .oneshot(request("GET", "/health/ready"))
                .await
                .unwrap()
                .status(),
            ready
        );
    }
    for target in ["/", "/wp-admin/", "/settings", "/publish", "/php"] {
        assert_eq!(
            management_router()
                .oneshot(request("POST", target))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
}

#[test]
fn invalid_configured_authority_is_rejected() {
    for host in [
        "",
        "https://example.test",
        "user@example.test",
        "example.test/path",
        "example.test\r\n",
        "日本.test",
        "example.test:",
        "example.test:bad",
        "example.test:70000",
    ] {
        assert!(public_router(None, host).is_err(), "{host:?}");
    }
}

#[tokio::test]
async fn router_generated_method_errors_also_have_protection_headers() {
    for router in [app(), management_router()] {
        for target in ["/health/live", "/health/ready"] {
            let response = router
                .clone()
                .oneshot(request("POST", target))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store"
            );
            assert_eq!(
                response
                    .headers()
                    .get(header::X_CONTENT_TYPE_OPTIONS)
                    .unwrap(),
                "nosniff"
            );
        }
    }
}

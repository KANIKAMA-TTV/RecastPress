use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use recastpress_core::{
    publication::{PublicPage, RasterImageKind, publication_channel},
    resource_budget::{BudgetPool, ResourceKind},
};
use recastpress_server::{bundle, public_router, public_router_with_budgeted_cache};
use serde_json::{Value, json};
use tower::ServiceExt;

const STATUSES: [u16; 5] = [301, 302, 303, 307, 308];
const LOCATION: &str = "/new/%7Eowner?q=one+two&q=%41&next=/local";

fn redirect(target: &str, status: u16, location: &str) -> Value {
    json!({"target":target,"status":status,"location":location})
}

fn text(target: &str) -> Value {
    json!({"target":target,"status":200,"content_type":"text/plain; charset=utf-8","body_hex":"626f647900"})
}

fn wire(pages: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"format_version":2,"pages":pages})).unwrap()
}

fn request(method: &str, target: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(target)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap()
}

#[test]
fn all_five_redirect_records_are_explicit_and_legacy_body_constructors_stay_closed() {
    let pages = STATUSES
        .iter()
        .map(|status| redirect(&format!("/old{status}"), *status, LOCATION))
        .chain([text("/body")])
        .collect();
    let bytes = wire(pages);
    let original = bytes.clone();
    let site = bundle::parse(&bytes).unwrap();
    assert_eq!(bytes, original);
    let (publisher, reader) = publication_channel();
    publisher
        .publish(&publisher.revision().unwrap(), site)
        .unwrap();
    drop(publisher);
    for status in STATUSES {
        let response = reader.read(&format!("/old{status}")).unwrap();
        let page = response.page();
        assert_eq!(page.status(), status);
        assert_eq!(page.location(), Some(LOCATION));
        assert_eq!(page.content_type(), "text/plain; charset=utf-8");
        assert!(page.body().is_empty());
        assert!(PublicPage::new(status, "text/plain; charset=utf-8".into(), vec![]).is_err());
        assert!(
            PublicPage::new_image(status, RasterImageKind::Png, b"\x89PNG\r\n\x1a\n".to_vec())
                .is_err()
        );
        let v1 = json!({"format_version":1,"pages":[redirect("/old",status,LOCATION)]});
        assert!(bundle::parse(&serde_json::to_vec(&v1).unwrap()).is_err());
        // Empty legacy bodies cannot become redirects without the typed record.
        let mut legacy = text("/old");
        legacy["status"] = json!(status);
        legacy["body_hex"] = json!("");
        assert!(bundle::parse(&wire(vec![legacy])).is_err());
    }
    let body = reader.read("/body").unwrap();
    assert_eq!(body.page().location(), None);
    assert_eq!(body.page().body(), b"body\0");
}

#[test]
fn redirect_variants_reject_duplicates_null_unknown_fields_and_body_shape_collisions() {
    let good = String::from_utf8(wire(vec![redirect("/old", 302, LOCATION)])).unwrap();
    for (key, value) in [
        ("target", "\"/old\""),
        ("status", "302"),
        ("location", "\"/new\""),
    ] {
        let needle = format!("\"{key}\":");
        for prefix in [value, "null"] {
            let duplicate = good.replacen(&needle, &format!("\"{key}\":{prefix},{needle}"), 1);
            assert!(
                bundle::parse(duplicate.as_bytes()).is_err(),
                "duplicate {key}, first {prefix}"
            );
        }
    }
    for key in ["target", "status", "location"] {
        let mut missing = redirect("/old", 302, LOCATION);
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            bundle::parse(&wire(vec![missing])).is_err(),
            "missing {key}"
        );
        let mut null = redirect("/old", 302, LOCATION);
        null[key] = Value::Null;
        assert!(bundle::parse(&wire(vec![null])).is_err(), "null {key}");
    }
    for key in [
        "body",
        "body_hex",
        "content_type",
        "headers",
        "file",
        "url",
        "php",
        "kind",
    ] {
        let mut object = redirect("/old", 302, LOCATION);
        object[key] = json!("ignored?");
        assert!(bundle::parse(&wire(vec![object])).is_err(), "extra {key}");
    }
    let mut both = text("/old");
    both["location"] = json!(LOCATION);
    both["status"] = json!(302);
    assert!(bundle::parse(&wire(vec![both])).is_err());
    let mut body_location = text("/body");
    body_location["location"] = json!(LOCATION);
    assert!(bundle::parse(&wire(vec![body_location])).is_err());
    for status in [
        json!(200),
        json!(300),
        json!(304),
        json!(305),
        json!(306),
        json!(309),
        json!(-1),
        json!(65536),
        json!(302.0),
        json!(true),
        json!("302"),
    ] {
        let mut object = redirect("/old", 302, LOCATION);
        object["status"] = status;
        assert!(bundle::parse(&wire(vec![object])).is_err());
    }
    for location in [json!(true), json!(1), json!(1.0), json!([]), json!({})] {
        let mut object = redirect("/old", 302, LOCATION);
        object["location"] = location;
        assert!(bundle::parse(&wire(vec![object])).is_err());
    }
    for key in ["format_version", "pages"] {
        let value = if key == "pages" { "[]" } else { "2" };
        let needle = format!("\"{key}\":");
        assert!(
            bundle::parse(
                good.replacen(&needle, &format!("\"{key}\":{value},{needle}"), 1)
                    .as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn redirect_manifest_binds_location_validation_and_keeps_route_rejections() {
    for location in [
        "",
        "relative",
        "?q=1",
        "#fragment",
        "http://example.test/",
        "https://evil.test/",
        "//evil.test/",
        "/\\evil.test/",
        "/bad\r\nLocation: //evil.test/",
        "/bad path",
        "/é",
        "/%",
        "/%GG",
        "/%00",
        "/%20",
        "/%7f",
        "/%2f%2Fevil.test/",
        "/%5c",
    ] {
        assert!(
            bundle::parse(&wire(vec![redirect("/old", 302, location)])).is_err(),
            "{location:?}"
        );
    }
    for target in [
        "//evil.test/",
        "relative",
        "/bad#fragment",
        "/health/live",
        "/health/ready?q=1",
    ] {
        assert!(
            bundle::parse(&wire(vec![redirect(target, 302, "/new")])).is_err(),
            "{target}"
        );
    }
    assert!(bundle::parse(&wire(vec![redirect("/same", 302, "/new"), text("/same")])).is_err());
    // No implicit destination-existence or graph-termination policy: these
    // records are static response instructions, never server-side fetches.
    assert!(
        bundle::parse(&wire(vec![
            redirect("/self", 302, "/self"),
            redirect("/unresolved", 307, "/not-listed")
        ]))
        .is_ok()
    );
    let bad_body = json!({"target":"/bad","status":200,"content_type":"text/plain; charset=utf-8","body_hex":"00g0"});
    assert!(
        bundle::parse(&wire(vec![redirect("/old", 302, "/new"), bad_body]))
            .unwrap_err()
            .contains("ASCII hex digits")
    );
}

#[tokio::test]
async fn redirect_get_head_have_one_exact_location_empty_body_and_no_implicit_follow() {
    let pages = STATUSES
        .iter()
        .map(|status| redirect(&format!("/old{status}?v=1"), *status, LOCATION))
        .chain([
            text("/body"),
            redirect("/to-body", 302, "/body"),
            redirect("/to-missing", 307, "/not-listed"),
        ])
        .collect();
    let app = public_router(Some(bundle::parse(&wire(pages)).unwrap()), "example.test").unwrap();
    for status in STATUSES {
        for method in ["GET", "HEAD"] {
            let response = app
                .clone()
                .oneshot(request(method, &format!("/old{status}?v=1")))
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
            assert_eq!(
                response.headers().get_all(header::LOCATION).iter().count(),
                1
            );
            assert_eq!(response.headers()[header::LOCATION], LOCATION);
            assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "text/plain; charset=utf-8"
            );
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                "nosniff"
            );
            assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
        }
    }
    for (target, location, status) in [
        ("/to-body", "/body", 302),
        ("/to-missing", "/not-listed", 307),
    ] {
        let response = app.clone().oneshot(request("GET", target)).await.unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(response.headers()[header::LOCATION], location);
        assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
    }
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/not-listed"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let body = app.clone().oneshot(request("GET", "/body")).await.unwrap();
    assert!(!body.headers().contains_key(header::LOCATION));
    assert_eq!(
        to_bytes(body.into_body(), 10).await.unwrap().as_ref(),
        b"body\0"
    );
    for method in ["POST", "PUT", "DELETE"] {
        let response = app
            .clone()
            .oneshot(request(method, "/to-body"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()[header::ALLOW], "GET, HEAD");
        assert!(!response.headers().contains_key(header::LOCATION));
    }
    let mut wrong = request("GET", "/to-body");
    wrong
        .headers_mut()
        .insert(header::HOST, "evil.test".parse().unwrap());
    for (request, expected) in [
        (wrong, StatusCode::MISDIRECTED_REQUEST),
        (request("GET", "/missing"), StatusCode::NOT_FOUND),
        (request("GET", "/old301"), StatusCode::NOT_FOUND),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
        assert!(!response.headers().contains_key(header::LOCATION));
    }
}

#[tokio::test]
async fn redirect_requests_leave_cache_and_pool_unchanged_with_or_without_global_capacity() {
    for capacity in [0, 65536] {
        let pool = BudgetPool::new(capacity);
        let pages = STATUSES
            .iter()
            .map(|status| redirect(&format!("/old{status}"), *status, "/body"))
            .chain([text("/body")])
            .collect();
        let (app, control) = public_router_with_budgeted_cache(
            Some(bundle::parse(&wire(pages)).unwrap()),
            "example.test",
            pool.clone(),
        )
        .unwrap();
        assert!(control.apply_allocation(65536));
        assert!(control.reconciliation_completed());
        let response = app.clone().oneshot(request("GET", "/body")).await.unwrap();
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            b"body\0"
        );
        let before = control.status().unwrap();
        let budget_before = pool.stats().unwrap();
        assert_eq!(before.entries, usize::from(capacity != 0));
        if capacity != 0 {
            assert!(budget_before.claimed_for(ResourceKind::ResponseCache) > 0);
        } else {
            assert_eq!(budget_before.claimed, 0);
        }
        for status in STATUSES {
            for method in ["GET", "HEAD"] {
                let response = app
                    .clone()
                    .oneshot(request(method, &format!("/old{status}")))
                    .await
                    .unwrap();
                assert_eq!(response.status().as_u16(), status);
                assert_eq!(control.status().unwrap(), before);
                assert_eq!(pool.stats().unwrap(), budget_before);
                assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
                assert_eq!(control.status().unwrap(), before);
                assert_eq!(pool.stats().unwrap(), budget_before);
            }
        }
        let response = app.oneshot(request("GET", "/body")).await.unwrap();
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap().as_ref(),
            b"body\0"
        );
        assert_eq!(control.status().unwrap().entries, before.entries);
        assert_eq!(pool.stats().unwrap(), budget_before);
        assert!(control.purge());
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use recastpress_core::{
    publication::{PreparedSite, PublicReader, publication_channel},
    resource_budget::{BudgetPool, ResourceKind},
};
use recastpress_server::{bundle, public_router, public_router_with_budgeted_cache};
use serde_json::{Value, json};
use tower::ServiceExt;

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [
                DIGITS[(byte >> 4) as usize] as char,
                DIGITS[(byte & 15) as usize] as char,
            ]
        })
        .collect()
}

fn page(target: &str, content_type: &str, body: &[u8]) -> Value {
    json!({"target":target,"status":200,"content_type":content_type,"body_hex":hex(body)})
}

fn wire(pages: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"format_version":2,"pages":pages})).unwrap()
}

fn reader(site: PreparedSite) -> PublicReader {
    let (publisher, reader) = publication_channel();
    publisher
        .publish(&publisher.revision().unwrap(), site)
        .unwrap();
    drop(publisher);
    reader
}

fn synthetic_png() -> Vec<u8> {
    // Signature fixture only: this is deliberately not a decoded/valid PNG.
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(0..=255);
    bytes
}

fn request(method: &str, target: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(target)
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .unwrap()
}

fn claim(pool: &BudgetPool) -> usize {
    pool.stats()
        .unwrap()
        .claimed_for(ResourceKind::ResponseCache)
}

#[test]
fn version_one_stays_text_only_and_version_two_preserves_text_and_raster_bytes() {
    let legacy=br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"old\u0000body"}]}"#;
    assert_eq!(
        reader(bundle::parse(legacy).unwrap())
            .read("/")
            .unwrap()
            .page()
            .body(),
        b"old\0body"
    );
    for mime in ["image/png", "image/jpeg", "image/webp"] {
        let v1 = json!({"format_version":1,"pages":[{"target":"/","status":200,"content_type":mime,"body":"text"}]});
        assert!(bundle::parse(&serde_json::to_vec(&v1).unwrap()).is_err());
    }
    let png = synthetic_png();
    let jpeg = b"\xff\xd8\xff\0\xfe\xff";
    let webp = b"RIFF\0\xff\x80\0WEBPVP\xff\0";
    let mut pages = vec![
        page("/png", "image/png", &png),
        page("/jpg", "image/jpeg", jpeg),
        page("/webp", "image/webp", webp),
    ];
    let text = "\0日本語\n".as_bytes();
    for (index, mime) in [
        "text/html; charset=utf-8",
        "text/plain; charset=utf-8",
        "application/rss+xml; charset=utf-8",
        "application/atom+xml; charset=utf-8",
    ]
    .iter()
    .enumerate()
    {
        let mut entry = page(&format!("/text{index}"), mime, text);
        // Uppercase hex digits are accepted without changing the decoded body.
        entry["body_hex"] = json!(hex(text).to_ascii_uppercase());
        pages.push(entry);
    }
    pages.push(page("/empty", "text/plain; charset=utf-8", b""));
    let bytes = wire(pages);
    let original = bytes.clone();
    let prepared = reader(bundle::parse(&bytes).unwrap());
    assert_eq!(bytes, original);
    for (target, mime, body) in [
        ("/png", "image/png", png.as_slice()),
        ("/jpg", "image/jpeg", jpeg.as_slice()),
        ("/webp", "image/webp", webp.as_slice()),
    ] {
        let response = prepared.read(target).unwrap();
        assert_eq!(response.page().content_type(), mime);
        assert_eq!(response.page().body(), body);
    }
    for index in 0..4 {
        assert_eq!(
            prepared
                .read(&format!("/text{index}"))
                .unwrap()
                .page()
                .body(),
            text
        );
    }
    assert!(prepared.read("/empty").unwrap().page().body().is_empty());
}

#[test]
fn both_versions_reject_ambiguous_unknown_null_and_malformed_manifest_fields() {
    let good =
        String::from_utf8(wire(vec![page("/", "text/plain; charset=utf-8", b"ok")])).unwrap();
    for (key, value) in [("format_version", "2"), ("pages", "[]")] {
        let needle = format!("\"{key}\":");
        let duplicate = good.replacen(&needle, &format!("\"{key}\":{value},{needle}"), 1);
        assert!(
            bundle::parse(duplicate.as_bytes()).is_err(),
            "duplicate {key}"
        );
    }
    for (key, value) in [
        ("target", "\"/\""),
        ("status", "200"),
        ("content_type", "\"text/plain; charset=utf-8\""),
        ("body_hex", "\"6f6b\""),
    ] {
        let needle = format!("\"{key}\":");
        let duplicate = good.replacen(&needle, &format!("\"{key}\":{value},{needle}"), 1);
        assert!(
            bundle::parse(duplicate.as_bytes()).is_err(),
            "duplicate {key}"
        );
    }
    for key in ["target", "status", "content_type", "body_hex"] {
        for replacement in [None, Some(Value::Null)] {
            let mut object = page("/", "text/plain; charset=utf-8", b"ok");
            if let Some(value) = replacement {
                object[key] = value;
            } else {
                object.as_object_mut().unwrap().remove(key);
            }
            assert!(
                bundle::parse(&wire(vec![object])).is_err(),
                "missing/null {key}"
            );
        }
    }
    for invalid in [
        r#"{"format_version":2}"#,
        r#"{"pages":[]}"#,
        r#"{"format_version":2,"pages":null}"#,
        r#"{"format_version":2,"pages":{}}"#,
        r#"{"format_version":2,"pages":[null]}"#,
        r#"{"format_version":2,"pages":[]} {}"#,
    ] {
        assert!(bundle::parse(invalid.as_bytes()).is_err(), "{invalid}");
    }
    for value in [json!(true), json!(12), json!(12.0), json!([]), json!({})] {
        let mut object = page("/", "text/plain; charset=utf-8", b"ok");
        object["body_hex"] = value;
        assert!(bundle::parse(&wire(vec![object])).is_err());
    }
    for key in ["body", "file", "path", "url", "source", "private"] {
        let mut object = page("/", "text/plain; charset=utf-8", b"ok");
        object[key] = json!("ignored?");
        assert!(bundle::parse(&wire(vec![object])).is_err(), "unknown {key}");
    }
    for version in [
        json!(0),
        json!(4),
        json!(-1),
        json!(2.0),
        json!(true),
        Value::Null,
        json!("2"),
    ] {
        let object =
            json!({"format_version":version,"pages":[page("/","text/plain; charset=utf-8",b"ok")]});
        assert!(bundle::parse(&serde_json::to_vec(&object).unwrap()).is_err());
    }
    for target in [
        "//evil.test/",
        "https://evil.test/",
        "relative",
        "/bad#fragment",
        "/bad\\path",
        "/health/live",
        "/health/ready?q=1",
    ] {
        assert!(
            bundle::parse(&wire(vec![page(
                target,
                "text/plain; charset=utf-8",
                b"ok"
            )]))
            .is_err(),
            "{target}"
        );
    }
    assert!(bundle::parse(&wire(vec![])).is_err());
    let duplicate = page("/", "text/plain; charset=utf-8", b"ok");
    assert!(bundle::parse(&wire(vec![duplicate.clone(), duplicate])).is_err());
    for mime in [
        "image/jpg",
        "image/PNG",
        "image/png; charset=utf-8",
        "image/svg+xml",
        "image/webp ",
        "application/octet-stream",
        "text/plain",
    ] {
        assert!(
            bundle::parse(&wire(vec![page("/", mime, &synthetic_png())])).is_err(),
            "{mime}"
        );
    }
    for status in [
        json!(204),
        json!(302),
        json!(200.0),
        json!(true),
        json!(-1),
        json!(65536),
    ] {
        let mut object = page("/", "text/plain; charset=utf-8", b"ok");
        object["status"] = status;
        assert!(bundle::parse(&wire(vec![object])).is_err());
    }
    let extra = good.replacen('{', "{\"external_fetch\":false,", 1);
    assert!(bundle::parse(extra.as_bytes()).is_err());
    assert!(
        bundle::parse(
            good.replace("\"body_hex\":\"6f6b\"", "\"body\":\"ok\"")
                .as_bytes()
        )
        .is_err()
    );
    assert!(
        bundle::parse(
            good.replace("\"format_version\":2", "\"format_version\":1")
                .as_bytes()
        )
        .is_err()
    );
    // The legacy body's unknown-field/duplicate protections survive dispatch.
    for legacy in [
        r#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":null}]}"#,
        r#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"a","body":"b"}]}"#,
        r#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"a","file":"secret"}]}"#,
    ] {
        assert!(bundle::parse(legacy.as_bytes()).is_err());
    }
}

#[test]
fn complete_hex_preflight_rejects_invalid_late_bodies_and_text_requires_utf8() {
    for invalid in [
        "0", "000", "0x00", " 00", "00\n", "0g", "gg", "＋00", "００",
    ] {
        let mut object = page("/", "text/plain; charset=utf-8", b"");
        object["body_hex"] = json!(invalid);
        assert!(
            bundle::parse(&wire(vec![object]))
                .unwrap_err()
                .contains("ASCII hex digits"),
            "{invalid}"
        );
    }
    for invalid in [
        b"\xff".as_slice(),
        b"\xc0\x80",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\xe2\x82",
    ] {
        assert!(
            bundle::parse(&wire(vec![page("/", "text/plain; charset=utf-8", invalid)]))
                .unwrap_err()
                .contains("not valid UTF-8")
        );
    }
    let mut late = page("/late", "text/plain; charset=utf-8", b"ok");
    late["body_hex"] = json!("00g0");
    let first = page("/invalid-signature", "image/png", b"not a PNG");
    // The late malformed wire is found before any earlier body's image check:
    // validation covers every hex body before decoding starts.
    assert!(
        bundle::parse(&wire(vec![first, late]))
            .unwrap_err()
            .contains("ASCII hex digits")
    );
}

#[test]
fn the_encoded_bundle_limit_is_aggregate_and_accepts_exactly_eight_mib() {
    let max = bundle::MAX_BUNDLE_BYTES as usize;
    let mut pages = vec![
        page("/first", "text/plain; charset=utf-8", b""),
        page("/second", "text/plain; charset=utf-8", b""),
    ];
    let overhead = wire(pages.clone()).len();
    let count = (max - overhead) / 4;
    pages[0]["body_hex"] = json!("61".repeat(count));
    pages[1]["body_hex"] = json!("62".repeat(count));
    let mut bytes = wire(pages);
    bytes.resize(max, b' ');
    let prepared = reader(bundle::parse(&bytes).unwrap());
    for (target, byte) in [("/first", b'a'), ("/second", b'b')] {
        let response = prepared.read(target).unwrap();
        assert_eq!(response.page().body().len(), count);
        assert!(response.page().body().iter().all(|actual| *actual == byte));
    }
    bytes.push(b' ');
    assert_eq!(bundle::parse(&bytes).unwrap_err(), "bundle exceeds 8 MiB");
}

#[tokio::test]
async fn binary_http_get_head_and_explicit_routes_preserve_bytes_and_protection_headers() {
    let png = synthetic_png();
    let bodies = [
        ("/png?v=1", "image/png", png.as_slice()),
        ("/jpeg", "image/jpeg", b"\xff\xd8\xff\x80\0".as_slice()),
        ("/webp", "image/webp", b"RIFF\0\0\0\0WEBPVP".as_slice()),
    ];
    let app = public_router(
        Some(
            bundle::parse(&wire(
                bodies
                    .iter()
                    .map(|(target, mime, body)| page(target, mime, body))
                    .collect(),
            ))
            .unwrap(),
        ),
        "example.test",
    )
    .unwrap();
    for (target, mime, body) in bodies {
        for method in ["GET", "HEAD"] {
            let response = app.clone().oneshot(request(method, target)).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
            assert_eq!(
                response.headers()[header::CONTENT_LENGTH],
                body.len().to_string()
            );
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                "nosniff"
            );
            let actual = to_bytes(response.into_body(), 1024).await.unwrap();
            assert_eq!(actual.as_ref(), if method == "GET" { body } else { b"" });
        }
    }
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/png"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/missing"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.clone()
            .oneshot(request("POST", "/jpeg"))
            .await
            .unwrap()
            .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    let mut wrong = request("GET", "/jpeg");
    wrong
        .headers_mut()
        .insert(header::HOST, "evil.test".parse().unwrap());
    assert_eq!(
        app.oneshot(wrong).await.unwrap().status(),
        StatusCode::MISDIRECTED_REQUEST
    );
}

#[tokio::test]
async fn binary_cache_owners_survive_purge_and_global_refusal_preserves_fallback() {
    let png = synthetic_png();
    let bytes = wire(vec![page("/png", "image/png", &png)]);
    let pool = BudgetPool::new(65536);
    let (app, control) = public_router_with_budgeted_cache(
        Some(bundle::parse(&bytes).unwrap()),
        "example.test",
        pool.clone(),
    )
    .unwrap();
    assert!(control.apply_allocation(65536));
    assert!(control.reconciliation_completed());
    let head = app.clone().oneshot(request("HEAD", "/png")).await.unwrap();
    assert!(to_bytes(head.into_body(), 0).await.unwrap().is_empty());
    assert_eq!(claim(&pool), 0);
    let first = app.clone().oneshot(request("GET", "/png")).await.unwrap();
    let charge = claim(&pool);
    assert!(charge > png.len());
    let second = app.clone().oneshot(request("GET", "/png")).await.unwrap();
    assert_eq!(claim(&pool), charge);
    assert!(control.purge());
    assert_eq!(control.status().unwrap().entries, 0);
    assert_eq!(claim(&pool), charge);
    drop(second);
    assert_eq!(claim(&pool), charge);
    let collected = to_bytes(first.into_body(), 1024).await.unwrap();
    assert_eq!(collected.as_ref(), png);
    assert_eq!(claim(&pool), charge);
    let cloned = collected.clone();
    let slice = collected.slice(8..);
    drop(collected);
    drop(cloned);
    assert_eq!(slice.as_ref(), &(0..=255).collect::<Vec<u8>>());
    assert_eq!(claim(&pool), charge);
    drop(slice);
    assert_eq!(claim(&pool), 0);
    pool.set_limit(0).unwrap();
    let before = control.status().unwrap();
    let fallback = app.oneshot(request("GET", "/png")).await.unwrap();
    assert_eq!(fallback.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(fallback.into_body(), 1024).await.unwrap().as_ref(),
        png
    );
    assert_eq!(control.status().unwrap(), before);
    assert_eq!(claim(&pool), 0);
}

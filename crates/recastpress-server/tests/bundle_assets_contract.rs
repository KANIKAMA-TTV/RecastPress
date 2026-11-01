//! Versioned asset admission with preserved legacy report bytes.
//! Uses current public bundle/receipt interfaces, no new dependency or Cargo fork.
use recastpress_core::publication::{ReadError, publication_channel};
use recastpress_server::{bundle, bundle_receipt};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

include!("support/assets_legacy_vectors.rs");

const OLD_MIMES: [&str; 7] = [
    "application/atom+xml; charset=utf-8",
    "application/rss+xml; charset=utf-8",
    "image/jpeg",
    "image/png",
    "image/webp",
    "text/html; charset=utf-8",
    "text/plain; charset=utf-8",
];
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02x}")).collect()
}
fn page(target: &str, mime: &str, raw: &[u8]) -> Value {
    json!({"target":target,"status":200,"content_type":mime,"body_hex":hex(raw)})
}
fn wire(pages: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"format_version":3,"pages":pages})).unwrap()
}

struct Owned {
    root: PathBuf,
    parent: PathBuf,
}
impl Owned {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .canonicalize()
            .unwrap();
        let root = parent.join(format!(
            "bundle-assets-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self { root, parent }
    }
    fn report(&self, raw: &[u8]) -> (Value, Vec<u8>) {
        let path = self.root.join("bundle.json");
        fs::write(&path, raw).unwrap();
        let report = bundle::check(&path).unwrap();
        let mut bytes = serde_json::to_vec(&report).unwrap();
        bytes.push(b'\n');
        assert!(bytes.len() <= 1024);
        assert_eq!(fs::read(&path).unwrap(), raw);
        (serde_json::to_value(report).unwrap(), bytes)
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        assert_eq!(
            self.root.canonicalize().unwrap().parent(),
            Some(self.parent.as_path())
        );
        assert!(
            !fs::symlink_metadata(&self.root)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn seven_actual_before_observer_legacy_inputs_preserve_acceptance_and_exact_schema1_report_bytes() {
    let owned = Owned::new();
    assert_eq!(LEGACY_BEFORE.len(), 7);
    for (name, input, accepted, stdout) in LEGACY_BEFORE {
        if *accepted {
            let (report, bytes) = owned.report(input);
            assert_eq!(bytes, *stdout, "legacy report changed: {name}");
            assert_eq!(report["format_version"], 1);
            assert_eq!(
                report["mime_counts"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                OLD_MIMES
            );
        } else {
            assert!(
                bundle::parse(input).is_err(),
                "legacy domain widened: {name}"
            );
            let path = owned.root.join("legacy-refused.json");
            fs::write(&path, input).unwrap();
            assert!(bundle::check(&path).is_err());
            assert!(stdout.is_empty());
        }
    }
}

#[test]
fn v3_html_css_js_and_redirect_report2_has_exact_nine_mimes_and_same_prepared_bytes() {
    let html=b"<!doctype html><link rel=stylesheet href='/theme.css?v=1'><script src='/theme.js?v=1'></script>";
    let css = "/* public */\r\nh1 { --label: '日本🦀'; }\n".as_bytes();
    let js = b"/* public */\r\ndocument.documentElement.dataset.fixture='ok';\n";
    let raw = wire(vec![
        page("/", OLD_MIMES[5], html),
        page("/theme.css?v=1", CSS, css),
        page("/theme.js?v=1", JS, js),
        json!({"target":"/old","status":308,"location":"/"}),
    ]);
    let owned = Owned::new();
    let (report, _) = owned.report(&raw);
    let mut keys = report
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "accepted_bundle_version",
            "body_bytes",
            "format_version",
            "mime_counts",
            "pages",
            "redirects"
        ]
    );
    assert_eq!(
        (
            report["format_version"].as_u64(),
            report["accepted_bundle_version"].as_u64()
        ),
        (Some(2), Some(3))
    );
    assert_eq!(
        (
            report["pages"].as_u64(),
            report["redirects"].as_u64(),
            report["body_bytes"].as_u64()
        ),
        (
            Some(4),
            Some(1),
            Some((html.len() + css.len() + js.len()) as u64)
        )
    );
    let mut expected = OLD_MIMES.to_vec();
    expected.extend([CSS, JS]);
    expected.sort_unstable();
    assert_eq!(
        report["mime_counts"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        expected
    );
    for mime in expected {
        assert_eq!(
            report["mime_counts"][mime],
            if [OLD_MIMES[5], CSS, JS].contains(&mime) {
                1
            } else {
                0
            }
        );
    }
    let (publisher, reader) = publication_channel();
    publisher
        .publish(&publisher.revision().unwrap(), bundle::parse(&raw).unwrap())
        .unwrap();
    for (target, mime, bytes) in [
        ("/", OLD_MIMES[5], html.as_slice()),
        ("/theme.css?v=1", CSS, css),
        ("/theme.js?v=1", JS, js.as_slice()),
    ] {
        let response = reader.read(target).unwrap();
        assert_eq!(response.page().content_type(), mime);
        assert_eq!(response.page().body(), bytes);
    }
    assert_eq!(
        reader.read("/theme.css?v=2").unwrap_err(),
        ReadError::NotFound
    );
}

#[test]
fn assets_require_status200_valid_utf8_exact_mime_without_legacy_fallback() {
    for mime in [CSS, JS] {
        for status in [0, 201, 301, 404, 410, 500] {
            let mut p = page("/asset", mime, b"public");
            p["status"] = json!(status);
            assert!(bundle::parse(&wire(vec![p])).is_err());
        }
        for invalid in [
            b"\xff".as_slice(),
            b"\xc0\xaf",
            b"\xed\xa0\x80",
            b"end\xe2\x82",
        ] {
            assert!(bundle::parse(&wire(vec![page("/asset", mime, invalid)])).is_err());
        }
        for version in [1, 2] {
            let p = if version == 1 {
                json!({"target":"/asset","status":200,"content_type":mime,"body":"public"})
            } else {
                page("/asset", mime, b"public")
            };
            assert!(
                bundle::parse(
                    &serde_json::to_vec(&json!({"format_version":version,"pages":[p]})).unwrap()
                )
                .is_err()
            );
        }
    }
    for mime in [
        "text/css",
        "text/javascript",
        "application/javascript",
        "TEXT/CSS; charset=utf-8",
        "text/css; charset=utf-8; extra=1",
        " text/javascript; charset=utf-8",
    ] {
        assert!(bundle::parse(&wire(vec![page("/asset", mime, b"public")])).is_err());
    }
}

#[test]
fn strict_v3_fields_hex_routes_and_versions_refuse_without_reinterpretation() {
    let good = page("/asset", CSS, b"public");
    let mut cases = vec![wire(vec![]), wire(vec![good.clone(), good.clone()])];
    for target in [
        "/health/live?x=1",
        "/health/ready",
        "//host",
        "/bad#x",
        "/bad space",
    ] {
        cases.push(wire(vec![page(target, CSS, b"public")]));
    }
    for (field, value) in [
        ("private", json!("PRIVATE")),
        ("body", json!("inconsistent")),
        ("location", json!("/else")),
        ("body_hex", json!("0")),
        ("body_hex", json!("zz")),
    ] {
        let mut p = good.clone();
        p[field] = value;
        cases.push(wire(vec![p]));
    }
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("content_type");
    cases.push(wire(vec![missing]));
    cases.push(br#"{"format_version":3,"format_version":3,"pages":[]}"#.to_vec());
    cases.push(br#"{"format_version":3,"pages":[{"target":"/a","target":"/b","status":200,"content_type":"text/css; charset=utf-8","body_hex":""}]}"#.to_vec());
    cases.push(serde_json::to_vec(&json!({"format_version":4,"pages":[good]})).unwrap());
    for raw in cases {
        assert!(bundle::parse(&raw).is_err());
    }
}

#[test]
fn v3_exact_8mib_wire_padding_and_sentinel_keep_same_byte_receipt1_identity() {
    let mut raw = wire(vec![page("/empty.css", CSS, b"")]);
    raw.resize(bundle::MAX_BUNDLE_BYTES as usize, b' ');
    let before = raw.clone();
    let receipt = bundle_receipt::receipt_from_payload(&raw).unwrap();
    let receipt_value: Value = serde_json::from_slice(&receipt).unwrap();
    assert_eq!(receipt_value["format_version"], 1);
    assert_eq!(receipt_value["payload_bytes"], bundle::MAX_BUNDLE_BYTES);
    assert!(receipt.len() <= bundle_receipt::MAX_RECEIPT_BYTES);
    assert!(bundle_receipt::verify_payload(&raw, &receipt).is_ok());
    assert_eq!(raw, before);
    raw.push(b' ');
    assert!(bundle::parse(&raw).is_err());
    assert!(bundle_receipt::receipt_from_payload(&raw).is_err());
    assert!(bundle_receipt::verify_payload(&raw, &receipt).is_err());
}

#[test]
fn receipt1_verified_check_reports_schema2_without_reencoding_payload_or_receipt() {
    let raw = wire(vec![
        page("/public.css", CSS, b"h1{color:teal}\r\n"),
        page("/public.js", JS, b"void 0;\n"),
    ]);
    let receipt = bundle_receipt::receipt_from_payload(&raw).unwrap();
    let owned = Owned::new();
    let bundle_path = owned.root.join("bundle.json");
    let receipt_path = owned.root.join("receipt.json");
    fs::write(&bundle_path, &raw).unwrap();
    fs::write(&receipt_path, &receipt).unwrap();
    let report = bundle_receipt::check_verified(&bundle_path, &receipt_path).unwrap();
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["format_version"], 2);
    assert_eq!(value["accepted_bundle_version"], 3);
    assert_eq!(fs::read(&bundle_path).unwrap(), raw);
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt);
    let mut changed = raw.clone();
    changed.push(b' ');
    assert!(bundle_receipt::verify_payload(&changed, &receipt).is_err());
}

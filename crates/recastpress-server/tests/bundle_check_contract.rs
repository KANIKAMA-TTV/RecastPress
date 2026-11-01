use recastpress_server::bundle;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(wire: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "recastpress-bundle-check-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, wire).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}
fn report(value: Value) -> Value {
    let fixture = Fixture::new(&serde_json::to_vec(&value).unwrap());
    serde_json::to_value(bundle::check(&fixture.0).unwrap()).unwrap()
}

#[test]
fn both_versions_share_admission_and_report_decoded_bytes_fixed_mimes_and_redirects() {
    let marker = "SECRET_BODY_日";
    let one = report(
        json!({"format_version":1,"pages":[{"target":"/secret-target","status":200,"content_type":"text/html; charset=utf-8","body":marker}]}),
    );
    assert_eq!(one["format_version"], 1);
    assert_eq!(one["accepted_bundle_version"], 1);
    assert_eq!(one["pages"], 1);
    assert_eq!(one["redirects"], 0);
    assert_eq!(one["body_bytes"], marker.len());
    assert_eq!(one["mime_counts"].as_object().unwrap().len(), 7);
    assert_eq!(one["mime_counts"]["text/html; charset=utf-8"], 1);
    let two = report(json!({"format_version":2,"pages":[
        {"target":"/opaque","status":200,"content_type":"image/png","body_hex":"89504e470d0a1a0a"},
        {"target":"/text","status":404,"content_type":"text/plain; charset=utf-8","body_hex":"00e697a5"},
        {"target":"/redirect","status":308,"location":"/private-marker?x=1&y=2"}]}));
    assert_eq!(two["accepted_bundle_version"], 2);
    assert_eq!(two["pages"], 3);
    assert_eq!(two["redirects"], 1);
    assert_eq!(two["body_bytes"], 12);
    assert_eq!(two["mime_counts"]["image/png"], 1);
    assert_eq!(two["mime_counts"]["text/plain; charset=utf-8"], 1);
    assert_eq!(
        two["mime_counts"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>(),
        2
    );
    for value in [one, two] {
        let wire = serde_json::to_string(&value).unwrap();
        assert!(wire.len() < 1024);
        assert!(!wire.contains("SECRET"));
        assert!(!wire.contains("secret-target"));
        assert!(!wire.contains("private-marker"));
    }
}

#[test]
fn check_never_reports_a_generation_rejected_by_the_existing_strict_loader() {
    let base =
        json!({"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"ok"});
    for value in [
        json!({"format_version":1,"pages":[]}),
        json!({"format_version":1,"pages":[base.clone(),base]}),
        json!({"format_version":1,"pages":[{"target":"/health/live?q=1","status":200,"content_type":"text/plain; charset=utf-8","body":"ok"}]}),
        json!({"format_version":2,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body_hex":"ff"}]}),
        json!({"format_version":2,"pages":[{"target":"/","status":200,"content_type":"image/png","body_hex":"00000000"}]}),
        json!({"format_version":2,"pages":[{"target":"/","status":302,"location":"https://other.test/"}]}),
        json!({"format_version":4,"pages":[]}),
    ] {
        let fixture = Fixture::new(&serde_json::to_vec(&value).unwrap());
        assert_eq!(
            bundle::check(&fixture.0).unwrap_err(),
            bundle::load(&fixture.0).unwrap_err()
        );
    }
    for wire in [
        br#"{"format_version":1,"format_version":1,"pages":[]}"#.as_slice(),
        br#"{"format_version":1,"pages":[],"SECRET_UNKNOWN":true}"#,
    ] {
        let fixture = Fixture::new(wire);
        assert!(bundle::check(&fixture.0).is_err());
    }
}

#[test]
fn check_retains_the_original_read_bound_and_reports_no_partial_summary() {
    let mut exact = br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"ok"}]}"#.to_vec();
    exact.resize(bundle::MAX_BUNDLE_BYTES as usize, b' ');
    let accepted = Fixture::new(&exact);
    assert_eq!(
        serde_json::to_value(bundle::check(&accepted.0).unwrap()).unwrap()["body_bytes"],
        2
    );
    assert!(bundle::load(&accepted.0).is_ok());
    let fixture = Fixture::new(&vec![b' '; bundle::MAX_BUNDLE_BYTES as usize + 1]);
    assert_eq!(
        bundle::check(&fixture.0).unwrap_err(),
        "bundle exceeds 8 MiB"
    );
    assert_eq!(
        bundle::check(&fixture.0).unwrap_err(),
        bundle::load(&fixture.0).unwrap_err()
    );
}

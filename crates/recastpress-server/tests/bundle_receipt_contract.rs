use recastpress_server::{
    bundle,
    bundle_receipt::{self, BundleReceiptError as Error, MAX_RECEIPT_BYTES},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const V1: &[u8] = br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"SECRET_BODY"}]}"#;
const SHA: &str = "603460c90b82319b2648365c7ea9d23e9ed9eb14db797e9df48872e3b8a9b47d";
fn receipt(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap()
}
fn valid() -> Value {
    json!({"format_version":1,"payload_bytes":122,"sha256":SHA})
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files(Vec<PathBuf>);
impl Files {
    fn new(payload: &[u8], receipt: &[u8]) -> Self {
        let name = format!(
            "bundle-receipt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let paths = vec![
            std::env::temp_dir().join(format!("{name}-SECRET_PAYLOAD.json")),
            std::env::temp_dir().join(format!("{name}-SECRET_RECEIPT.json")),
        ];
        std::fs::write(&paths[0], payload).unwrap();
        std::fs::write(&paths[1], receipt).unwrap();
        Self(paths)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        for path in &self.0 {
            if path.exists() {
                std::fs::remove_file(path).unwrap();
            }
        }
    }
}

#[test]
fn issued_receipts_are_deterministic_exact_bytes_with_independent_sha256_and_no_body() {
    // hashlib.sha256 of the fixed 122 input bytes above, independently observed.
    let output = bundle_receipt::receipt_from_payload(V1).unwrap();
    assert_eq!(output, bundle_receipt::receipt_from_payload(V1).unwrap());
    assert!(output.ends_with(b"\n"));
    assert!(output.len() <= MAX_RECEIPT_BYTES);
    assert_eq!(serde_json::from_slice::<Value>(&output).unwrap(), valid());
    assert!(
        !String::from_utf8(output.clone())
            .unwrap()
            .contains("SECRET")
    );
    assert_eq!(
        bundle_receipt::verify_payload(V1, &output)
            .unwrap()
            .targets()
            .collect::<Vec<_>>(),
        ["/"]
    );
    let mut whitespace = V1.to_vec();
    whitespace.push(b' ');
    let other = bundle_receipt::receipt_from_payload(&whitespace).unwrap();
    assert_ne!(other, output);
    assert_eq!(
        bundle_receipt::verify_payload(&whitespace, &output).unwrap_err(),
        Error::PayloadLengthMismatch
    );
    let v2=br#"{"format_version":2,"pages":[{"target":"/png","status":200,"content_type":"image/png","body_hex":"89504e470d0a1a0a"},{"target":"/old","status":302,"location":"/new"}]}"#;
    let receipt = bundle_receipt::receipt_from_payload(v2).unwrap();
    assert_eq!(
        bundle_receipt::verify_payload(v2, &receipt)
            .unwrap()
            .targets()
            .collect::<Vec<_>>(),
        ["/old", "/png"]
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&receipt)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn receipt_schema_rejects_duplicates_unknown_fields_types_versions_and_noncanonical_digests() {
    for (value, expected_error) in [
        (
            json!({"format_version":2,"payload_bytes":122,"sha256":SHA}),
            Error::ReceiptVersion,
        ),
        (
            json!({"format_version":true,"payload_bytes":122,"sha256":SHA}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1.0,"payload_bytes":122,"sha256":SHA}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":true,"sha256":SHA}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":122.0,"sha256":SHA}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":"122","sha256":SHA}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":-1,"sha256":SHA}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":122,"sha256":null}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":122,"sha256":SHA,"SECRET_UNKNOWN":"SECRET_BODY"}),
            Error::ReceiptSyntax,
        ),
        (
            json!({"format_version":1,"payload_bytes":122}),
            Error::ReceiptSyntax,
        ),
    ] {
        let error = bundle_receipt::verify_payload(V1, &receipt(value)).unwrap_err();
        assert_eq!(error, expected_error);
        assert!(!format!("{error:?} {error}").contains("SECRET"));
    }
    for key in ["format_version", "payload_bytes", "sha256"] {
        let value = valid();
        let body = serde_json::to_string(&value).unwrap();
        let duplicate = format!("{{\"{key}\":{},{}", value[key], &body[1..]);
        assert_eq!(
            bundle_receipt::verify_payload(V1, duplicate.as_bytes()).unwrap_err(),
            Error::ReceiptSyntax
        );
    }
    for digest in [
        SHA.to_ascii_uppercase(),
        "g".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        "é".repeat(32),
        format!("{}\0", &SHA[..63]),
    ] {
        assert_eq!(
            bundle_receipt::verify_payload(
                V1,
                &receipt(json!({"format_version":1,"payload_bytes":122,"sha256":digest}))
            )
            .unwrap_err(),
            Error::ReceiptDigest
        );
    }
}

#[test]
fn matching_identity_does_not_replace_strict_admission_and_bad_identity_is_typed() {
    let mut changed = V1.to_vec();
    let position = changed.iter().position(|b| *b == b'S').unwrap();
    changed[position] = b'X';
    assert_eq!(
        bundle_receipt::verify_payload(&changed, &receipt(valid())).unwrap_err(),
        Error::PayloadDigestMismatch
    );
    let invalid = b"not JSON";
    let identity = receipt(
        json!({"format_version":1,"payload_bytes":8,"sha256":"62b8125a6f6d924ec53345b5fcd58ca3ed3f5e7d51e2e146e5f1346508acce69"}),
    );
    assert_eq!(
        bundle_receipt::verify_payload(invalid, &identity).unwrap_err(),
        Error::BundleAdmission
    );
    assert_eq!(
        bundle_receipt::receipt_from_payload(invalid).unwrap_err(),
        Error::BundleAdmission
    );
    let mut too_long = V1.to_vec();
    too_long.push(b' ');
    assert_eq!(
        bundle_receipt::verify_payload(&too_long, &receipt(valid())).unwrap_err(),
        Error::PayloadLengthMismatch
    );
}

#[test]
fn exact_receipt_and_payload_bounds_are_independent_and_one_byte_over_is_refused() {
    let mut wire = receipt(valid());
    wire.resize(MAX_RECEIPT_BYTES, b' ');
    assert!(bundle_receipt::verify_payload(V1, &wire).is_ok());
    wire.push(b' ');
    assert_eq!(
        bundle_receipt::verify_payload(V1, &wire).unwrap_err(),
        Error::ReceiptInputLimit
    );
    let mut payload = V1.to_vec();
    payload.resize(bundle::MAX_BUNDLE_BYTES as usize, b' ');
    let identity = bundle_receipt::receipt_from_payload(&payload).unwrap();
    assert!(bundle_receipt::verify_payload(&payload, &identity).is_ok());
    payload.push(b' ');
    assert_eq!(
        bundle_receipt::verify_payload(&payload, &identity).unwrap_err(),
        Error::PayloadInputLimit
    );
    assert_eq!(
        bundle_receipt::receipt_from_payload(&payload).unwrap_err(),
        Error::PayloadInputLimit
    );
}

#[test]
fn actual_files_are_admitted_once_and_later_mutation_or_io_failure_cannot_change_the_owned_site() {
    let identity = bundle_receipt::receipt_from_payload(V1).unwrap();
    let files = Files::new(V1, &identity);
    assert_eq!(
        bundle_receipt::receipt_from_file(&files.0[0]).unwrap(),
        identity
    );
    let site = bundle_receipt::load_verified(&files.0[0], &files.0[1]).unwrap();
    let (publisher, reader) = recastpress_core::publication::publication_channel();
    publisher
        .publish(&publisher.revision().unwrap(), site)
        .unwrap();
    std::fs::write(
        &files.0[0],
        V1.iter()
            .map(|b| if *b == b'S' { b'X' } else { *b })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(reader.read("/").unwrap().page().body(), b"SECRET_BODY");
    assert_eq!(
        bundle_receipt::load_verified(&files.0[0], &files.0[1]).unwrap_err(),
        Error::PayloadDigestMismatch
    );
    std::fs::write(
        &files.0[0],
        vec![b' '; bundle::MAX_BUNDLE_BYTES as usize + 1],
    )
    .unwrap();
    assert_eq!(
        bundle_receipt::load_verified(&files.0[0], &files.0[1]).unwrap_err(),
        Error::PayloadInputLimit
    );
    std::fs::write(&files.0[1], vec![b' '; MAX_RECEIPT_BYTES + 1]).unwrap();
    assert_eq!(
        bundle_receipt::load_verified(&files.0[0], &files.0[1]).unwrap_err(),
        Error::ReceiptInputLimit
    );
    std::fs::remove_file(&files.0[1]).unwrap();
    assert_eq!(
        bundle_receipt::load_verified(&files.0[0], &files.0[1]).unwrap_err(),
        Error::ReceiptIo
    );
    std::fs::write(&files.0[1], &identity).unwrap();
    std::fs::remove_file(&files.0[0]).unwrap();
    assert_eq!(
        bundle_receipt::load_verified(&files.0[0], &files.0[1]).unwrap_err(),
        Error::PayloadIo
    );
}

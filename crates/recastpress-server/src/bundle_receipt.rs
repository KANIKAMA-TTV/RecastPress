//! Experimental byte-identity receipt for an existing v1/v2/v3 prepared bundle.
//!
//! Receipt schema 1 contains only payload length and SHA256. It is not a
//! signature, source authentication, public-content approval, anti-rollback
//! mechanism, publication Revision or a product file/IPC protocol decision.
//! A party able to replace payload and receipt can generate a matching pair.
//! No references inside either input cause filesystem/network/DB/PHP access.

use crate::bundle;
use recastpress_core::publication::PreparedSite;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, fs::File, io::Read, path::Path};

pub const MAX_RECEIPT_BYTES: usize = 1024;

/// Closed refusals: neither Debug nor Display retain body, path, input digest
/// or arbitrary parser/IO diagnostics. Allocation/RSS/time bounds are not claimed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleReceiptError {
    PayloadInputLimit,
    ReceiptInputLimit,
    ReceiptSyntax,
    ReceiptVersion,
    ReceiptDigest,
    PayloadLengthMismatch,
    PayloadDigestMismatch,
    BundleAdmission,
    PayloadIo,
    ReceiptIo,
    ReceiptSerialization,
}
impl fmt::Display for BundleReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PayloadInputLimit => "bundle payload exceeds the input bound",
            Self::ReceiptInputLimit => "bundle receipt exceeds the input bound",
            Self::ReceiptSyntax => "bundle receipt schema is invalid",
            Self::ReceiptVersion => "bundle receipt version is unsupported",
            Self::ReceiptDigest => "bundle receipt digest encoding is invalid",
            Self::PayloadLengthMismatch => "bundle payload length does not match receipt",
            Self::PayloadDigestMismatch => "bundle payload digest does not match receipt",
            Self::BundleAdmission => "bundle strict admission failed",
            Self::PayloadIo => "bundle payload could not be read",
            Self::ReceiptIo => "bundle receipt could not be read",
            Self::ReceiptSerialization => "bundle receipt could not be encoded",
        })
    }
}
impl std::error::Error for BundleReceiptError {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    format_version: u32,
    payload_bytes: u64,
    sha256: String,
}

fn check_payload_bound(payload: &[u8]) -> Result<(), BundleReceiptError> {
    if payload.len() as u64 > bundle::MAX_BUNDLE_BYTES {
        return Err(BundleReceiptError::PayloadInputLimit);
    }
    Ok(())
}

fn digest_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0u8; 64];
    for (index, byte) in digest.iter().copied().enumerate() {
        bytes[index * 2] = HEX[usize::from(byte >> 4)];
        bytes[index * 2 + 1] = HEX[usize::from(byte & 15)];
    }
    // Only the fixed ASCII alphabet above enters this conversion.
    String::from_utf8(bytes.to_vec()).expect("generated digest is ASCII")
}

fn parse_receipt(bytes: &[u8]) -> Result<Receipt, BundleReceiptError> {
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err(BundleReceiptError::ReceiptInputLimit);
    }
    let receipt: Receipt =
        serde_json::from_slice(bytes).map_err(|_| BundleReceiptError::ReceiptSyntax)?;
    if receipt.format_version != 1 {
        return Err(BundleReceiptError::ReceiptVersion);
    }
    if receipt.sha256.len() != 64
        || !receipt
            .sha256
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(BundleReceiptError::ReceiptDigest);
    }
    Ok(receipt)
}

/// Issue a deterministic receipt only after the complete existing strict bundle
/// admission succeeds. Covers exact input bytes, including JSON whitespace and
/// key order, not normalized semantic identity. Does not write or publish data.
pub fn receipt_from_payload(payload: &[u8]) -> Result<Vec<u8>, BundleReceiptError> {
    check_payload_bound(payload)?;
    bundle::parse(payload).map_err(|_| BundleReceiptError::BundleAdmission)?;
    let receipt = Receipt {
        format_version: 1,
        payload_bytes: payload.len() as u64,
        sha256: digest_hex(payload),
    };
    let mut wire =
        serde_json::to_vec(&receipt).map_err(|_| BundleReceiptError::ReceiptSerialization)?;
    wire.push(b'\n');
    if wire.len() > MAX_RECEIPT_BYTES {
        return Err(BundleReceiptError::ReceiptSerialization);
    }
    Ok(wire)
}

/// Read one explicit payload path with the existing sentinel bound and issue
/// a receipt only after its same-buffer strict admission succeeds.
pub fn receipt_from_file(payload_path: &Path) -> Result<Vec<u8>, BundleReceiptError> {
    let payload = bundle::read_payload(payload_path).map_err(|_| BundleReceiptError::PayloadIo)?;
    receipt_from_payload(&payload)
}

/// Verify exact byte length and SHA256 before admitting the same immutable
/// borrowed buffer. Receipt syntax/identity errors never call bundle admission.
/// All strict v1/v2/v3 limits remain; receipt identity does not make invalid public
/// content admissible. No file or URL references in the bytes are followed.
pub fn verify_payload(payload: &[u8], receipt: &[u8]) -> Result<PreparedSite, BundleReceiptError> {
    verify_with_admission(payload, receipt, bundle::parse)
}

fn verify_with_admission<T>(
    payload: &[u8],
    receipt: &[u8],
    admit: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, BundleReceiptError> {
    check_payload_bound(payload)?;
    let receipt = parse_receipt(receipt)?;
    if payload.len() as u64 != receipt.payload_bytes {
        return Err(BundleReceiptError::PayloadLengthMismatch);
    }
    if digest_hex(payload) != receipt.sha256 {
        return Err(BundleReceiptError::PayloadDigestMismatch);
    }
    admit(payload).map_err(|_| BundleReceiptError::BundleAdmission)
}

fn read_receipt(reader: impl Read) -> Result<Vec<u8>, BundleReceiptError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_RECEIPT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| BundleReceiptError::ReceiptIo)?;
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err(BundleReceiptError::ReceiptInputLimit);
    }
    Ok(bytes)
}

/// Read each explicit path once with a sentinel limit, then hash and strictly
/// admit the same owned payload buffer. File changes after this read cannot
/// alter its PreparedSite; no live watcher, atomic file-pair update, durable
/// write protocol, symlink policy, authenticity or rollback prevention is added.
pub fn load_verified(
    payload_path: &Path,
    receipt_path: &Path,
) -> Result<PreparedSite, BundleReceiptError> {
    load_with_admission(payload_path, receipt_path, bundle::parse)
}

/// Verify the receipt before the same-buffer strict v1/v2/v3 admission and return
/// only the existing content-free summary. Reads receipt first, then payload,
/// once each; no listener, publication, directory enumeration or referenced
/// asset access occurs. Like `load_verified`, this does not add atomic file-pair
/// reads, symlink isolation, source authentication or rollback prevention.
pub fn check_verified(
    payload_path: &Path,
    receipt_path: &Path,
) -> Result<bundle::BundleCheckReport, BundleReceiptError> {
    load_with_admission(payload_path, receipt_path, bundle::check_payload)
}

fn load_with_admission<T>(
    payload_path: &Path,
    receipt_path: &Path,
    admit: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, BundleReceiptError> {
    let receipt =
        read_receipt(File::open(receipt_path).map_err(|_| BundleReceiptError::ReceiptIo)?)?;
    // Reject malformed receipts before allocating/reading the payload. Parsing
    // again in verify_with_admission is bounded and never reopens either file.
    parse_receipt(&receipt)?;
    let payload = bundle::read_payload(payload_path).map_err(|_| BundleReceiptError::PayloadIo)?;
    verify_with_admission(&payload, &receipt, admit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct PairFiles {
        payload: std::path::PathBuf,
        receipt: std::path::PathBuf,
    }
    impl PairFiles {
        fn new(payload: &[u8], receipt: &[u8]) -> Self {
            let name = format!(
                "check-package-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let paths = Self {
                payload: std::env::temp_dir().join(format!("{name}.bundle")),
                receipt: std::env::temp_dir().join(format!("{name}.receipt")),
            };
            std::fs::write(&paths.payload, payload).unwrap();
            std::fs::write(&paths.receipt, receipt).unwrap();
            paths
        }
    }
    impl Drop for PairFiles {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.payload);
            let _ = std::fs::remove_file(&self.receipt);
        }
    }
    fn identity_receipt(payload: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&Receipt {
            format_version: 1,
            payload_bytes: payload.len() as u64,
            sha256: digest_hex(payload),
        })
        .unwrap()
    }

    #[test]
    fn verified_summaries_keep_v1_v2_mime_and_redirect_counts_without_content() {
        for (payload, version, pages, redirects, body_bytes, mime) in [
            (
                br#"{"format_version":1,"pages":[{"target":"/PRIVATE_ROUTE","status":200,"content_type":"text/plain; charset=utf-8","body":"PRIVATE_BODY"}]}"#.as_slice(),
                1, 1, 0, 12, "text/plain; charset=utf-8",
            ),
            (
                br#"{"format_version":2,"pages":[{"target":"/PRIVATE_IMAGE","status":200,"content_type":"image/png","body_hex":"89504e470d0a1a0a"},{"target":"/PRIVATE_REDIRECT","status":308,"location":"/PRIVATE_DESTINATION"}]}"#.as_slice(),
                2, 2, 1, 8, "image/png",
            ),
        ] {
            let receipt = receipt_from_payload(payload).unwrap();
            let files = PairFiles::new(payload, &receipt);
            let report = check_verified(&files.payload, &files.receipt).unwrap();
            assert_eq!(report, bundle::check(&files.payload).unwrap());
            let json = serde_json::to_value(&report).unwrap();
            let mut counts = serde_json::json!({
                "text/html; charset=utf-8": 0, "text/plain; charset=utf-8": 0,
                "application/rss+xml; charset=utf-8": 0,
                "application/atom+xml; charset=utf-8": 0,
                "image/png": 0, "image/jpeg": 0, "image/webp": 0
            });
            counts[mime] = serde_json::json!(1);
            assert_eq!(json, serde_json::json!({
                "format_version": 1, "accepted_bundle_version": version,
                "pages": pages, "redirects": redirects, "body_bytes": body_bytes,
                "mime_counts": counts
            }));
            let wire = serde_json::to_string(&report).unwrap();
            assert!(!wire.contains("PRIVATE"));
            assert!(!wire.contains("sha256"));
            assert!(!wire.contains(&files.payload.to_string_lossy().to_string()));
            assert_eq!(std::fs::read(&files.payload).unwrap(), payload);
            assert_eq!(std::fs::read(&files.receipt).unwrap(), receipt);
            assert_eq!(load_verified(&files.payload, &files.receipt).unwrap().targets().count(), pages);
        }
    }

    #[test]
    fn checked_summary_uses_one_verified_buffer_and_never_admits_bad_identity() {
        let payload = br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"A"}]}"#;
        let receipt = identity_receipt(payload);
        let files = PairFiles::new(payload, &receipt);
        let mut calls = 0;
        let report = load_with_admission(&files.payload, &files.receipt, |buffer| {
            calls += 1;
            // Both path contents change after the verified owned read. This
            // must not replace the buffer or cause either file to be reopened.
            std::fs::write(&files.payload, b"PRIVATE_NOT_A_BUNDLE").unwrap();
            std::fs::write(&files.receipt, b"PRIVATE_NOT_A_RECEIPT").unwrap();
            assert_eq!(buffer, payload);
            bundle::check_payload(buffer)
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(serde_json::to_value(report).unwrap()["body_bytes"], 1);
        assert_eq!(
            check_verified(&files.payload, &files.receipt).unwrap_err(),
            BundleReceiptError::ReceiptSyntax
        );
        let changed = payload
            .iter()
            .map(|byte| if *byte == b'A' { b'B' } else { *byte })
            .collect::<Vec<_>>();
        assert_eq!(
            verify_with_admission::<bundle::BundleCheckReport>(&changed, &receipt, |_| panic!(
                "mismatched digest reached admission"
            ))
            .unwrap_err(),
            BundleReceiptError::PayloadDigestMismatch
        );
        assert_eq!(
            verify_with_admission::<bundle::BundleCheckReport>(b"short", &receipt, |_| panic!(
                "mismatched length reached admission"
            ))
            .unwrap_err(),
            BundleReceiptError::PayloadLengthMismatch
        );
        let invalid = br#"{"format_version":1,"pages":[]}"#;
        assert_eq!(
            verify_with_admission(invalid, &identity_receipt(invalid), bundle::check_payload)
                .unwrap_err(),
            BundleReceiptError::BundleAdmission
        );
        std::fs::remove_file(&files.payload).unwrap();
        // Receipt parsing precedes even an attempt to open the absent payload.
        assert_eq!(
            check_verified(&files.payload, &files.receipt).unwrap_err(),
            BundleReceiptError::ReceiptSyntax
        );
    }

    #[test]
    fn verified_summary_accepts_exact_bounds_and_rejects_both_sentinels() {
        let small = br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"A"}]}"#;
        let mut exact = vec![b' '; bundle::MAX_BUNDLE_BYTES as usize];
        exact[..small.len()].copy_from_slice(small);
        let mut receipt = identity_receipt(&exact);
        receipt.resize(MAX_RECEIPT_BYTES, b' ');
        let files = PairFiles::new(&exact, &receipt);
        assert_eq!(
            serde_json::to_value(check_verified(&files.payload, &files.receipt).unwrap()).unwrap()
                ["body_bytes"],
            1
        );
        exact.push(b' ');
        std::fs::write(&files.payload, &exact).unwrap();
        assert_eq!(
            check_verified(&files.payload, &files.receipt).unwrap_err(),
            BundleReceiptError::PayloadInputLimit
        );
        receipt.push(b' ');
        std::fs::write(&files.receipt, receipt).unwrap();
        std::fs::remove_file(&files.payload).unwrap();
        assert_eq!(
            check_verified(&files.payload, &files.receipt).unwrap_err(),
            BundleReceiptError::ReceiptInputLimit
        );
    }

    #[test]
    fn file_mutation_after_hash_cannot_change_the_single_admitted_buffer_and_bad_identity_never_admits()
     {
        let payload = br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"A"}]}"#;
        let changed = br#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/plain; charset=utf-8","body":"B"}]}"#;
        let name = format!(
            "receipt-same-buffer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let payload_path = std::env::temp_dir().join(format!("{name}.json"));
        let receipt_path = std::env::temp_dir().join(format!("{name}.receipt"));
        struct Files(Vec<std::path::PathBuf>);
        impl Drop for Files {
            fn drop(&mut self) {
                for path in &self.0 {
                    std::fs::remove_file(path).unwrap();
                }
            }
        }
        let _cleanup = Files(vec![payload_path.clone(), receipt_path.clone()]);
        std::fs::write(&payload_path, payload).unwrap();
        let receipt = receipt_from_payload(payload).unwrap();
        std::fs::write(&receipt_path, &receipt).unwrap();
        let mut calls = 0;
        let site = load_with_admission(&payload_path, &receipt_path, |buffer| {
            calls += 1;
            std::fs::write(&payload_path, changed).unwrap();
            assert_eq!(buffer, payload);
            assert_eq!(digest_hex(buffer), parse_receipt(&receipt).unwrap().sha256);
            bundle::parse(buffer)
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(site.targets().collect::<Vec<_>>(), ["/"]);
        assert_eq!(std::fs::read(&payload_path).unwrap(), changed);
        assert_eq!(
            load_verified(&payload_path, &receipt_path).unwrap_err(),
            BundleReceiptError::PayloadDigestMismatch
        );
        assert_eq!(
            verify_with_admission::<PreparedSite>(changed, &receipt, |_| panic!(
                "bad identity reached admission"
            ))
            .unwrap_err(),
            BundleReceiptError::PayloadDigestMismatch
        );
    }

    #[test]
    fn bounded_receipt_reader_stops_at_one_sentinel_and_preserves_typed_failures() {
        let mut reader = std::io::Cursor::new(vec![b'x'; MAX_RECEIPT_BYTES + 32]);
        assert_eq!(
            read_receipt(&mut reader).unwrap_err(),
            BundleReceiptError::ReceiptInputLimit
        );
        assert_eq!(reader.position(), (MAX_RECEIPT_BYTES + 1) as u64);
        let exact = vec![b' '; MAX_RECEIPT_BYTES];
        assert_eq!(read_receipt(exact.as_slice()).unwrap(), exact);
        struct Denied;
        impl Read for Denied {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "SECRET_PATH",
                ))
            }
        }
        assert_eq!(
            read_receipt(Denied).unwrap_err(),
            BundleReceiptError::ReceiptIo
        );
    }
}

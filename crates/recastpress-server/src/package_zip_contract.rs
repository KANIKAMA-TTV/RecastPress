use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn archive(payload: &[u8], receipt: &[u8]) -> Vec<u8> {
    // Test-only encoder. Python zipfile + actual JS/native relation is separate.
    let entries = [
        (b"bundle.json".as_slice(), payload),
        (b"receipt.json".as_slice(), receipt),
    ];
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    for (name, body) in entries {
        offsets.push(bytes.len() as u32);
        bytes.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        for n in [20_u16, 0, 0, 0, 33] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        for n in [crc32(body), body.len() as u32, body.len() as u32] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(body);
    }
    let central_start = bytes.len();
    for ((name, body), offset) in entries.into_iter().zip(offsets) {
        bytes.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        for n in [20_u16, 20, 0, 0, 0, 33] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        for n in [crc32(body), body.len() as u32, body.len() as u32] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        for n in [name.len() as u16, 0, 0, 0, 0] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(name);
    }
    let central_size = bytes.len() - central_start;
    bytes.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    for n in [0_u16, 0, 2, 2] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    bytes.extend_from_slice(&(central_size as u32).to_le_bytes());
    bytes.extend_from_slice(&(central_start as u32).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes
}

struct OwnedRoot {
    parent: PathBuf,
    path: PathBuf,
}
impl OwnedRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let workspace = if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
            // Normal server Cargo tests run from the crate directory, not root.
            Path::new(manifest)
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf()
        } else {
            // Standalone rustc draft execution is explicitly from workspace root.
            let root = std::env::current_dir().unwrap();
            assert!(
                root.join("target/workbench-package-import-draft/standalone.rs")
                    .is_file()
            );
            root
        };
        assert!(workspace.join("Cargo.toml").is_file());
        let parent = workspace.join("target").canonicalize().unwrap();
        let path = parent.join(format!(
            "package-zip-unit-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { parent, path }
    }
    fn source(&self, bytes: &[u8]) -> PathBuf {
        let path = self.path.join("PRIVATE-archive.zip");
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for OwnedRoot {
    fn drop(&mut self) {
        let path = self.path.canonicalize().unwrap();
        assert_eq!(path.parent(), Some(self.parent.as_path()));
        assert!(
            !fs::symlink_metadata(&self.path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn canonical_pair_borrows_exact_bytes_without_mutation_and_crc_matches_standard_vectors() {
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    assert_eq!(crc32(b""), 0);
    let bytes = archive(b"PUBLIC\0\r\n\xff", b"receipt bytes\n");
    let original = bytes.clone();
    let pair = parse(&bytes).unwrap();
    assert_eq!(pair.payload, b"PUBLIC\0\r\n\xff");
    assert_eq!(pair.receipt, b"receipt bytes\n");
    assert_eq!(pair.payload.as_ptr(), bytes[41..].as_ptr());
    assert_eq!(bytes.len(), pair.payload.len() + pair.receipt.len() + 220);
    assert_eq!(bytes, original);
}

#[test]
fn every_truncation_header_byte_identity_change_and_extra_byte_is_rejected() {
    let bytes = archive(b"payload", b"receipt");
    for end in 0..bytes.len() {
        assert!(parse(&bytes[..end]).is_err(), "truncation {end}");
    }
    // Every byte is significant: fixed fields, both names, CRC, sizes, offsets,
    // content and central records. No ignored comments/attrs/gaps/padding.
    for index in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert!(parse(&changed).is_err(), "changed byte {index}");
    }
    let mut appended = bytes.clone();
    appended.push(0);
    assert!(parse(&appended).is_err());
    let mut prefixed = vec![0];
    prefixed.extend_from_slice(&bytes);
    assert!(parse(&prefixed).is_err());
    // Even coherent local+central CRC declarations cannot replace actual CRC.
    let mut forged = bytes.clone();
    let central = 41 + b"payload".len() + 42 + b"receipt".len();
    let wrong = (crc32(b"payload") ^ 1).to_le_bytes();
    forged[14..18].copy_from_slice(&wrong);
    forged[central + 16..central + 20].copy_from_slice(&wrong);
    assert!(parse(&forged).is_err());
}

#[test]
fn exact_payload_receipt_and_archive_caps_and_oversized_declarations_are_bounded() {
    let payload = vec![b'x'; MAX_PAYLOAD_BYTES];
    let receipt = vec![b'r'; MAX_RECEIPT_BYTES];
    let bytes = archive(&payload, &receipt);
    assert_eq!(bytes.len(), MAX_ARCHIVE_BYTES);
    let pair = parse(&bytes).unwrap();
    assert_eq!(pair.payload.len(), MAX_PAYLOAD_BYTES);
    assert_eq!(pair.receipt.len(), MAX_RECEIPT_BYTES);
    let mut plus = bytes;
    plus.push(0);
    assert!(parse(&plus).is_err());
    let mut declared = archive(b"x", b"r");
    declared[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
    declared[22..26].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(parse(&declared).is_err());
    let too_large_receipt = archive(b"", &vec![b'r'; MAX_RECEIPT_BYTES + 1]);
    assert!(parse(&too_large_receipt).is_err());
}

#[test]
fn trusted_admission_receives_same_borrowed_pair_before_any_output_and_success_preserves_files() {
    let root = OwnedRoot::new();
    let zip = archive(b"PUBLIC", b"PUBLIC receipt");
    let input = root.source(&zip);
    let output = root.path.join("new");
    let mut calls = 0;
    unpack_with(
        &input,
        &output,
        |payload, receipt| {
            calls += 1;
            assert!(!output.exists());
            assert_eq!(payload, b"PUBLIC");
            assert_eq!(receipt, b"PUBLIC receipt");
            Ok(())
        },
        write_new,
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(fs::read(output.join("bundle.json")).unwrap(), b"PUBLIC");
    assert_eq!(
        fs::read(output.join("receipt.json")).unwrap(),
        b"PUBLIC receipt"
    );
    assert_eq!(fs::read(input).unwrap(), zip);
    let mut names: Vec<_> = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["bundle.json", "receipt.json"]);
}

#[test]
fn malformed_zip_and_failed_native_admission_do_not_create_output_or_call_writer() {
    let root = OwnedRoot::new();
    let output = root.path.join("new");
    let invalid = root.source(b"PRIVATE invalid ZIP");
    assert!(
        unpack_with(
            &invalid,
            &output,
            |_, _| panic!("invalid ZIP reached admission"),
            |_, _| panic!("unexpected write")
        )
        .is_err()
    );
    assert!(!output.exists());
    let valid = root.source(&archive(b"payload", b"receipt"));
    assert!(
        unpack_with(
            &valid,
            &output,
            |_, _| Err(PackageZipError(())),
            |_, _| panic!("failed admission reached write")
        )
        .is_err()
    );
    assert!(!output.exists());
}

#[test]
fn existing_destination_missing_parent_and_nonregular_source_are_refused_without_changes() {
    let root = OwnedRoot::new();
    let zip = archive(b"payload", b"receipt");
    let input = root.source(&zip);
    let existing = root.path.join("existing");
    fs::write(&existing, b"KEEP").unwrap();
    assert!(
        unpack_with(
            &input,
            &existing,
            |_, _| panic!("existing destination reached admission"),
            write_new
        )
        .is_err()
    );
    assert_eq!(fs::read(existing).unwrap(), b"KEEP");
    assert!(
        unpack_with(
            &input,
            &root.path.join("missing/child"),
            |_, _| panic!("missing parent reached admission"),
            write_new
        )
        .is_err()
    );
    assert!(
        unpack_with(
            &root.path,
            &root.path.join("new"),
            |_, _| panic!("directory source reached admission"),
            write_new
        )
        .is_err()
    );
    assert!(!root.path.join("new").exists());
    assert_eq!(fs::read(input).unwrap(), zip);
}

#[test]
fn failed_second_write_retains_partial_output_and_fixed_errors_do_not_expose_diagnostics() {
    let root = OwnedRoot::new();
    let input = root.source(&archive(b"PUBLIC", b"receipt"));
    let output = root.path.join("new");
    let mut calls = 0;
    let error = unpack_with(
        &input,
        &output,
        |_, _| Ok(()),
        |path, bytes| {
            calls += 1;
            if calls == 1 {
                write_new(path, bytes)
            } else {
                fs::write(path, b"partial")?;
                Err(io::Error::other("PRIVATE IO detail"))
            }
        },
    )
    .unwrap_err();
    assert_eq!(calls, 2);
    assert_eq!(fs::read(output.join("bundle.json")).unwrap(), b"PUBLIC");
    assert_eq!(fs::read(output.join("receipt.json")).unwrap(), b"partial");
    assert_eq!(format!("{error:?}"), "PackageZipError");
    assert_eq!(error.to_string(), "package unpack failed");
    assert!(std::error::Error::source(&error).is_none());
    assert!(
        unpack_with(
            &input,
            &output,
            |_, _| panic!("retained output reached admission"),
            write_new
        )
        .is_err()
    );
}

#[test]
fn oversized_input_is_refused_before_admission_and_selected_paths_never_come_from_archive_names() {
    let root = OwnedRoot::new();
    let bytes = vec![0; MAX_ARCHIVE_BYTES + 1];
    let input = root.source(&bytes);
    let output = root.path.join("new");
    assert!(
        unpack_with(
            &input,
            &output,
            |_, _| panic!("oversized source reached admission"),
            write_new
        )
        .is_err()
    );
    let mut named = archive(b"PUBLIC", b"receipt");
    named[30..41].copy_from_slice(b"../evil.txt");
    root.source(&named);
    assert!(
        unpack_with(
            &input,
            &output,
            |_, _| panic!("archive filename reached admission"),
            write_new
        )
        .is_err()
    );
    assert!(!output.exists());
    assert!(!root.path.join("evil.txt").exists());
}

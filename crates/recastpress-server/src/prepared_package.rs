//! Offline creation of a new, explicitly selected article preparation package.
//!
//! Existing strict input, fixed literal-text renderer and bundle/receipt formats
//! are reused. No DB selection, theme execution, auth, listener or publication.
//! The directory is created only after complete validation. Failures retain any
//! partial directory/files; callers must not equate directory presence to success.

use crate::{bundle, bundle_receipt, prepared_posts};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

/// Opaque fixed failure: no path, input, digest or parser/IO source is retained.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PreparedPackageError(());
impl fmt::Debug for PreparedPackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedPackageError")
    }
}
impl fmt::Display for PreparedPackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("post package preparation failed")
    }
}
impl std::error::Error for PreparedPackageError {}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    // Drop closes the handle, but std does not expose close errors. The write
    // and sync calls above are observed; no explicit close ACK is claimed.
    drop(file);
    Ok(())
}

fn prepare_with(
    input: &Path,
    output_new_dir: &Path,
    mut write: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> Result<(), PreparedPackageError> {
    let file = File::open(input).map_err(|_| PreparedPackageError(()))?;
    let mut bytes = Vec::new();
    file.take(prepared_posts::MAX_POSTS_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PreparedPackageError(()))?;
    if bytes.len() as u64 > prepared_posts::MAX_POSTS_INPUT_BYTES {
        return Err(PreparedPackageError(()));
    }
    let payload = prepared_posts::prepare_bundle(
        &bytes,
        bundle::MAX_BUNDLE_BYTES as usize,
        bundle::MAX_BUNDLE_BYTES as usize,
    )
    .map_err(|_| PreparedPackageError(()))?;
    drop(bytes);
    let receipt =
        bundle_receipt::receipt_from_payload(&payload).map_err(|_| PreparedPackageError(()))?;
    // No output artifact exists before both in-memory artifacts are complete.
    // create_dir (not create_dir_all) refuses any existing final entry.
    fs::create_dir(output_new_dir).map_err(|_| PreparedPackageError(()))?;
    write(&output_new_dir.join("bundle.json"), &payload).map_err(|_| PreparedPackageError(()))?;
    write(&output_new_dir.join("receipt.json"), &receipt).map_err(|_| PreparedPackageError(()))?;
    Ok(())
}

/// Read one explicit input with the existing 8 MiB + sentinel bound; finish
/// strict preparation/admission and a receipt of the same immutable payload
/// buffer, then create a new directory with bundle.json and receipt.json.
/// Both files use create-new/write-all/sync-all and are dropped before success.
/// Parent directories are not created. No existing final file/dir/symlink is
/// overwritten; any failure retains partial artifacts without automatic cleanup.
///
/// There is no atomic pair/rename protocol, parent-directory durability,
/// close-error acknowledgement, hostile ancestor/symlink-race resolution,
/// signature/authenticity/latest-generation or production deployment contract.
/// Prepared payloads still require explicit public selection; receipt identity
/// does not approve content. Input/render/receipt/allocator work is unpooled,
/// and final byte limits are not RSS/peak/concurrency or complete allocation bounds.
pub fn prepare_post_package(
    input: &Path,
    output_new_dir: &Path,
) -> Result<(), PreparedPackageError> {
    prepare_with(input, output_new_dir, write_new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        error::Error,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct OwnedRoot {
        path: PathBuf,
        parent: PathBuf,
    }
    impl OwnedRoot {
        fn new() -> Self {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            let name = format!(
                "recastpress-package-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = parent.join(name);
            fs::create_dir(&path).unwrap();
            let path = path.canonicalize().unwrap();
            assert_eq!(path.parent(), Some(parent.as_path()));
            Self { path, parent }
        }
        fn join(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }
    impl Drop for OwnedRoot {
        fn drop(&mut self) {
            // Only this newly created absolute temp root is recursively removed.
            assert_eq!(self.path.parent(), Some(self.parent.as_path()));
            assert!(
                self.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("recastpress-package-")
            );
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
    fn input() -> Vec<u8> {
        "{\"format_version\":1,\"posts\":[{\"target\":\"/chosen\",\"title\":\"公開 & title\",\"body_text\":\"<script>literal</script>\\r\\nbody\"}]}".as_bytes().to_vec()
    }
    fn expected(bytes: &[u8]) -> Vec<u8> {
        prepared_posts::prepare_bundle(
            bytes,
            bundle::MAX_BUNDLE_BYTES as usize,
            bundle::MAX_BUNDLE_BYTES as usize,
        )
        .unwrap()
    }

    #[test]
    fn successful_new_package_has_exactly_two_existing_format_artifacts_and_valid_same_buffer_receipt()
     {
        let root = OwnedRoot::new();
        let source = root.join("input.json");
        let out = root.join("package");
        let bytes = input();
        fs::write(&source, &bytes).unwrap();
        prepare_post_package(&source, &out).unwrap();
        let payload = fs::read(out.join("bundle.json")).unwrap();
        let receipt = fs::read(out.join("receipt.json")).unwrap();
        assert_eq!(payload, expected(&bytes));
        assert_eq!(
            receipt,
            bundle_receipt::receipt_from_payload(&payload).unwrap()
        );
        assert!(bundle_receipt::verify_payload(&payload, &receipt).is_ok());
        let mut names = fs::read_dir(&out)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            vec![
                std::ffi::OsString::from("bundle.json"),
                std::ffi::OsString::from("receipt.json")
            ]
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }

    #[test]
    fn input_validation_render_and_admission_failures_never_create_the_output_directory() {
        let root = OwnedRoot::new();
        let source = root.join("PRIVATE-input.json");
        let mut cases=vec![
            b"not-json PRIVATE_SECRET".to_vec(),
            br#"{"format_version":1,"posts":[],"PRIVATE_SECRET":true}"#.to_vec(),
            br#"{"format_version":1,"format_version":1,"posts":[]}"#.to_vec(),
            br#"{"format_version":1,"posts":[]}"#.to_vec(),
            br#"{"format_version":1,"posts":[{"target":"/ok","title":"PRIVATE_SECRET\u0000","body_text":"text"}]}"#.to_vec(),
            br#"{"format_version":1,"posts":[{"target":"/health/live","title":"x","body_text":"x"}]}"#.to_vec(),
            br#"{"format_version":1,"posts":[{"target":"https://example.invalid/","title":"x","body_text":"x"}]}"#.to_vec(),
            br#"{"format_version":1,"posts":[{"target":"/same","title":"a","body_text":"a"},{"target":"/same","title":"b","body_text":"b"}]}"#.to_vec(),
        ];
        cases.push(serde_json::to_vec(&serde_json::json!({"format_version":1,"posts":[{"target":"/oversize","title":"","body_text":"&".repeat(bundle::MAX_BUNDLE_BYTES as usize/5+1)}]})).unwrap());
        for (index, bytes) in cases.iter().enumerate() {
            let out = root.join(&format!("package-{index}"));
            fs::write(&source, bytes).unwrap();
            let error = prepare_post_package(&source, &out).unwrap_err();
            assert!(!out.exists());
            assert_eq!(format!("{error:?}"), "PreparedPackageError");
            assert_eq!(error.to_string(), "post package preparation failed");
            assert!(error.source().is_none());
        }
        assert!(
            prepare_post_package(&root.join("missing-input"), &root.join("missing-package"))
                .is_err()
        );
        assert!(!root.join("missing-package").exists());
    }

    #[test]
    fn input_sentinel_accepts_exact_eight_mib_and_refuses_one_more_before_directory_creation() {
        let root = OwnedRoot::new();
        let source = root.join("input.json");
        let mut bytes = input();
        bytes.resize(prepared_posts::MAX_POSTS_INPUT_BYTES as usize, b' ');
        fs::write(&source, &bytes).unwrap();
        let exact = root.join("exact");
        prepare_post_package(&source, &exact).unwrap();
        assert!(
            bundle_receipt::verify_payload(
                &fs::read(exact.join("bundle.json")).unwrap(),
                &fs::read(exact.join("receipt.json")).unwrap()
            )
            .is_ok()
        );
        bytes.push(b' ');
        fs::write(&source, &bytes).unwrap();
        let over = root.join("over");
        assert!(prepare_post_package(&source, &over).is_err());
        assert!(!over.exists());
    }

    #[test]
    fn existing_entries_and_absent_parents_are_not_overwritten_or_automatically_created() {
        let root = OwnedRoot::new();
        let source = root.join("input.json");
        fs::write(&source, input()).unwrap();
        let existing = root.join("existing");
        fs::create_dir(&existing).unwrap();
        let sentinel = existing.join("bundle.json");
        fs::write(&sentinel, b"keep existing").unwrap();
        assert!(prepare_post_package(&source, &existing).is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep existing");
        assert!(write_new(&sentinel, b"must not replace").is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep existing");
        let file = root.join("file");
        fs::write(&file, b"keep file").unwrap();
        assert!(prepare_post_package(&source, &file).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"keep file");
        let parent = root.join("absent-parent");
        assert!(prepare_post_package(&source, &parent.join("package")).is_err());
        assert!(!parent.exists());
        let link = root.join("directory-link");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&existing, &link);
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_dir(&existing, &link);
        #[cfg(not(any(unix, windows)))]
        let linked: io::Result<()> = Err(io::ErrorKind::Unsupported.into());
        match linked {
            Ok(()) => {
                assert!(prepare_post_package(&source, &link).is_err());
                assert_eq!(fs::read(&sentinel).unwrap(), b"keep existing");
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
                ) || error.raw_os_error() == Some(1314) =>
            {
                eprintln!("owned symlink subcase unavailable: platform privilege required")
            }
            Err(_) => panic!("owned symlink fixture creation failed"),
        }
    }

    #[test]
    fn failed_second_write_retains_the_directory_first_file_and_partial_receipt_without_leaking_io_text()
     {
        let root = OwnedRoot::new();
        let source = root.join("input.json");
        let bytes = input();
        fs::write(&source, &bytes).unwrap();
        let out = root.join("partial");
        let mut calls = 0;
        let error = prepare_with(&source, &out, |path, data| {
            calls += 1;
            if calls == 1 {
                write_new(path, data)
            } else {
                let mut receipt = OpenOptions::new().write(true).create_new(true).open(path)?;
                receipt.write_all(b"partial")?;
                Err(io::Error::other("PRIVATE_IO_ERROR"))
            }
        })
        .unwrap_err();
        assert_eq!(calls, 2);
        assert!(out.is_dir());
        assert_eq!(fs::read(out.join("bundle.json")).unwrap(), expected(&bytes));
        assert_eq!(fs::read(out.join("receipt.json")).unwrap(), b"partial");
        assert_eq!(
            format!("{error:?}: {error}"),
            "PreparedPackageError: post package preparation failed"
        );
        assert!(prepare_post_package(&source, &out).is_err()); // no auto-resume/cleanup
        assert_eq!(fs::read(out.join("receipt.json")).unwrap(), b"partial");
    }

    #[test]
    fn source_mutation_after_validation_cannot_change_either_prepared_artifact() {
        let root = OwnedRoot::new();
        let source = root.join("input.json");
        let bytes = input();
        fs::write(&source, &bytes).unwrap();
        let out = root.join("same-buffer");
        let mut calls = 0;
        prepare_with(&source, &out, |path, data| {
            calls += 1;
            if calls == 1 {
                fs::write(&source, b"changed invalid source")?;
            }
            write_new(path, data)
        })
        .unwrap();
        assert_eq!(calls, 2);
        let payload = fs::read(out.join("bundle.json")).unwrap();
        let receipt = fs::read(out.join("receipt.json")).unwrap();
        assert_eq!(payload, expected(&bytes));
        assert!(bundle_receipt::verify_payload(&payload, &receipt).is_ok());
        assert_eq!(fs::read(&source).unwrap(), b"changed invalid source");
    }
}

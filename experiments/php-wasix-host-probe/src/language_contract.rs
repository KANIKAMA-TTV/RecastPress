//! One source-pinned CLI file, not a general PHP execution or host-FS API.

use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer_wasix::runners::wasi::WasiRunner;
use wasmer_wasix::virtual_fs::{
    DirEntry, FileOpener, FileSystem, FileType, FsError, Metadata, OpenOptions, OpenOptionsConfig,
    ReadDir, StaticFile, VirtualFile,
};

pub const FIXTURE_SHA256: &str = "5ab341e1207fc6bfb8634758bc38b6af7f6b60ba3c621c8acf52b603e5023080";
const FIXTURE: &[u8] =
    include_bytes!("../../php-runtime-candidate-probe/fixtures/runtime_contract.php");
const GUEST_PATH: &str = "/fixture/runtime_contract.php";
const CASES: [&str; 3] = ["basic", "exceptions", "fiber"];

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-language-contract.v0.5");
    report["scope"] = json!("fixed-fixture-basic-exceptions-fiber-only-no-product-integration");
    report["fixture"] = json!({
        "source": "experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php",
        "guest_path": GUEST_PATH,
        "expected_sha256": FIXTURE_SHA256,
        "verified": false,
        "storage": "compile-time-byte-snapshot-in-readonly-memory-filesystem"
    });
    report["comparison_scope"] = json!(CASES);
    report["uncompared_cases"] = json!(["wordpress_hooks", "scratch_io", "readonly_io"]);
    report["native_comparison_performed"] = json!(false);
    report["filesystem_contract_compared"] = json!(false);
    report["isolation"]["guest_readonly_mounts"] = json!(["/fixture"]);
    report["isolation"]["guest_fixture_files"] = json!([GUEST_PATH]);
    report["isolation"]["wordpress_directory_supplied"] = json!(false);
    report["isolation"]["scratch_directory_supplied"] = json!(false);
    report["isolation"]["guest_environment_supplied"] = json!([]);
    // WasiRunner still creates its own temporary virtual root. This mode only
    // establishes the immutable fixture mount; it is not a product FS boundary.
    report["isolation"]["other_virtual_root_writes_forbidden"] = json!(false);
    report["exception_support"]["execution_verdict"] = json!("not-yet-observed");
    report["limitations"] = json!([
        "Only basic, exceptions and fiber are compared by language_check.py against the native receipt.",
        "The unchanged fixture reports missing-directory errors for its three unprovided filesystem cases; these cases are not compared.",
        "No WordPress source or scratch directory is supplied, and no WordPress code is executed.",
        "The host observation alone does not establish a native comparison or full PHP compatibility.",
        "Memory, CPU and execution deadline enforcement remain the outer supervisor's responsibility."
    ]);
}

pub fn verify_fixture(report: &mut Value) -> Result<()> {
    report["phase"] = json!("language_fixture_verification");
    let digest = format!("{:x}", Sha256::digest(FIXTURE));
    report["fixture"]["observed_sha256"] = json!(digest);
    report["fixture"]["bytes"] = json!(FIXTURE.len());
    ensure!(
        digest == FIXTURE_SHA256,
        "fixed language fixture SHA-256 mismatch"
    );
    report["fixture"]["verified"] = json!(true);
    Ok(())
}

pub fn configure(runner: &mut WasiRunner) {
    runner
        .with_args(["-n", GUEST_PATH])
        .with_current_dir("/fixture")
        .with_mount("/fixture".to_owned(), Arc::new(FixtureFs));
}

pub fn validate_observation(php: &Value, report: &mut Value) -> Result<()> {
    report["phase"] = json!("language_observation_verification");
    ensure!(
        php["format_version"] == json!(1),
        "unexpected fixture format"
    );
    ensure!(
        php["runtime"]
            == json!({"version": "8.5.12", "version_id": 80512, "int_size": 8, "sapi": "cli"}),
        "unexpected PHP runtime identity"
    );
    ensure!(
        php["unexpected_output_hex"] == "",
        "unexpected buffered output"
    );
    let cases = php["cases"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("missing fixture cases"))?;
    ensure!(cases.len() == 6, "unexpected fixture case count");
    report["language_case_observations"] = json!(CASES);
    report["exceptions_executed"] = json!(
        cases
            .get("exceptions")
            .is_some_and(|case| case["status"] == "ok")
    );
    report["fiber_executed"] = json!(
        cases
            .get("fiber")
            .is_some_and(|case| case["status"] == "ok")
    );
    for name in CASES {
        let case = cases
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("missing case {name}"))?;
        ensure!(
            case["status"] == "ok",
            "language case {name} did not execute successfully"
        );
        ensure!(
            case["warnings"] == json!([]),
            "language case {name} emitted warnings"
        );
        ensure!(
            case.get("value").is_some(),
            "language case {name} omitted its value"
        );
    }
    report["exception_support"]["execution_verdict"] = json!("observed-not-yet-native-compared");
    Ok(())
}

/// The only exposed entries are / and /runtime_contract.php. All source bytes
/// were snapshotted at Rust compile time and verified before creating a runner.
#[derive(Debug)]
struct FixtureFs;

fn entry(path: &Path) -> std::result::Result<bool, FsError> {
    let mut name = None;
    for part in path.components() {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) if name.is_none() => name = Some(value),
            _ => return Err(FsError::EntryNotFound),
        }
    }
    match name {
        None => Ok(false),
        Some(name) if name == "runtime_contract.php" => Ok(true),
        _ => Err(FsError::EntryNotFound),
    }
}

fn metadata(file: bool) -> Metadata {
    Metadata {
        ft: FileType {
            file,
            dir: !file,
            ..FileType::default()
        },
        accessed: 0,
        created: 0,
        modified: 0,
        len: if file { FIXTURE.len() as u64 } else { 0 },
    }
}

impl FileOpener for FixtureFs {
    fn open(
        &self,
        path: &Path,
        options: &OpenOptionsConfig,
    ) -> std::result::Result<Box<dyn VirtualFile + Send + Sync>, FsError> {
        if options.would_mutate() {
            return Err(FsError::PermissionDenied);
        }
        if !entry(path)? {
            return Err(FsError::NotAFile);
        }
        Ok(Box::new(StaticFile::new(FIXTURE.to_vec())))
    }
}

impl FileSystem for FixtureFs {
    fn readlink(&self, path: &Path) -> std::result::Result<PathBuf, FsError> {
        entry(path)?;
        Err(FsError::InvalidInput)
    }

    fn read_dir(&self, path: &Path) -> std::result::Result<ReadDir, FsError> {
        if entry(path)? {
            return Err(FsError::BaseNotDirectory);
        }
        Ok(ReadDir::new(vec![DirEntry {
            path: PathBuf::from("/runtime_contract.php"),
            metadata: Ok(metadata(true)),
        }]))
    }

    fn metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        Ok(metadata(entry(path)?))
    }

    fn symlink_metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        self.metadata(path)
    }

    fn new_open_options(&self) -> OpenOptions<'_> {
        OpenOptions::new(self)
    }
    fn create_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn create_symlink(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn hard_link(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn remove_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn remove_file(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn rename<'a>(
        &'a self,
        _: &'a Path,
        _: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), FsError>> + Send + 'a>> {
        Box::pin(async { Err(FsError::PermissionDenied) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

    #[test]
    fn verified_source_is_the_only_readable_entry_and_handles_are_immutable() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let fs = FixtureFs;
            let mut report = json!({});
            verify_fixture(&mut report).unwrap();
            let mut file = fs
                .new_open_options()
                .read(true)
                .open("/runtime_contract.php")
                .unwrap();
            let mut observed = Vec::new();
            file.read_to_end(&mut observed).await.unwrap();
            assert_eq!(observed, FIXTURE);
            file.seek(std::io::SeekFrom::Start(0)).await.unwrap();
            assert_eq!(
                file.write(b"tampered").await.unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            assert_eq!(file.set_len(0), Err(FsError::PermissionDenied));
            assert_eq!(file.unlink(), Err(FsError::PermissionDenied));
            for path in [
                "/other.php",
                "/../runtime_contract.php",
                "/a/runtime_contract.php",
                "/runtime_contract.php/child",
            ] {
                assert!(
                    fs.new_open_options().read(true).open(path).is_err(),
                    "{path}"
                );
            }
        });
    }

    #[test]
    fn namespace_changes_and_mutating_open_modes_are_denied() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let fs = FixtureFs;
            let path = Path::new("/runtime_contract.php");
            for flags in [
                [true, false, false, false, false],
                [false, true, false, false, false],
                [false, false, true, false, false],
                [false, false, false, true, false],
                [false, false, false, false, true],
            ] {
                assert!(matches!(
                    fs.new_open_options()
                        .write(flags[0])
                        .append(flags[1])
                        .truncate(flags[2])
                        .create(flags[3])
                        .create_new(flags[4])
                        .open(path),
                    Err(FsError::PermissionDenied)
                ));
            }
            assert_eq!(fs.remove_file(path), Err(FsError::PermissionDenied));
            assert_eq!(
                fs.rename(path, Path::new("/moved")).await,
                Err(FsError::PermissionDenied)
            );
            assert_eq!(
                fs.create_dir(Path::new("/new")),
                Err(FsError::PermissionDenied)
            );
            assert_eq!(
                fs.hard_link(path, Path::new("/alias")),
                Err(FsError::PermissionDenied)
            );
            assert_eq!(
                fs.create_symlink(path, Path::new("/alias")),
                Err(FsError::PermissionDenied)
            );
        });
    }
}

//! Fixed owned Component HTML -> prepared bundle/receipt, offline only.
//! Not an adopted engine/SDK/ABI, content selector, authenticator, publisher or
//! GET-time renderer. No arbitrary module/input, WASI, host imports, PHP or DB.
//! Fuel excludes compilation/walltime. Store/copy limits are not whole RSS,
//! capacity/allocator/host/serialization/concurrency budgets. UTF-8 borrowing
//! before the owned copy applies only to this fixed canonical UTF-8 encoding.
#![forbid(unsafe_code)]

use recastpress_server::{bundle, bundle_receipt};
use std::{
    borrow::Cow,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};
use wasmtime::component::{Component, Linker, WasmStr};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder, Trap};

const HTML: &str = "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>Wasm fixture</title><body><h1>固定 Component</h1><p>Prepared before GET.</p></body></html>\n";
const TARGET: &str = "/wasm-fixture";
const MAX_HTML: usize = 4096;
const FUEL: u64 = 10_000;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PreparationError(());
impl fmt::Debug for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparationError")
    }
}
impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Wasm publication preparation failed")
    }
}
impl std::error::Error for PreparationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Summary {
    pub body_bytes: usize,
    pub bundle_bytes: usize,
    pub receipt_bytes: usize,
    pub post_return_completed: bool,
    pub guest_memory_changed_after_copy: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Guest,
    OutputLimit,
    Allocation,
    PostFuel,
    Profile,
    Admission,
    Io,
}
struct Rendered {
    bytes: Vec<u8>,
    post_return_completed: bool,
    guest_memory_changed_after_copy: bool,
}
struct Artifacts {
    payload: Vec<u8>,
    receipt: Vec<u8>,
    summary: Summary,
}

fn wat_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}
fn component_source(spinning_post: bool) -> String {
    let mut record = [0; 8];
    record[..4].copy_from_slice(&64u32.to_le_bytes());
    record[4..].copy_from_slice(&(HTML.len() as u32).to_le_bytes());
    let post = if spinning_post {
        "(loop $spin br $spin)"
    } else {
        "local.get 0 i32.const 8 i32.ne if unreachable end i32.const 64 i32.const 88 i32.store8"
    };
    format!(
        r#"(component
      (core module $m
        (memory (export "memory") 1)
        (data (i32.const 8) "{}") (data (i32.const 64) "{}")
        (func (export "render") (result i32) i32.const 8)
        (func (export "peek") (result i32) i32.const 8)
        (func (export "post") (param i32) {post}))
      (core instance $i (instantiate $m))
      (func (export "render") (result string)
        (canon lift (core func $i "render") (memory (core memory $i "memory")) (post-return (core func $i "post"))))
      (func (export "peek") (result string)
        (canon lift (core func $i "peek") (memory (core memory $i "memory")))))"#,
        wat_bytes(&record),
        wat_bytes(HTML.as_bytes())
    )
}
fn render_html(maximum: usize, spinning_post: bool) -> Result<Rendered, Refusal> {
    let mut configuration = Config::new();
    configuration.consume_fuel(true).wasm_component_model(true);
    let engine = Engine::new(&configuration).map_err(|_| Refusal::Guest)?;
    let component =
        Component::new(&engine, component_source(spinning_post)).map_err(|_| Refusal::Guest)?;
    let resources = component.resources_required().ok_or(Refusal::Profile)?;
    if resources.num_memories != 1
        || resources.max_initial_memory_size != Some(1)
        || resources.num_tables != 0
    {
        return Err(Refusal::Profile);
    }
    let limits = StoreLimitsBuilder::new()
        .instances(1)
        .memories(1)
        .tables(1)
        .memory_size(65_536)
        .table_elements(16)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|limits| limits);
    store.set_fuel(FUEL).map_err(|_| Refusal::Guest)?;
    let instance = Linker::new(&engine)
        .instantiate(&mut store, &component)
        .map_err(|_| Refusal::Guest)?;
    let render = instance
        .get_typed_func::<(), (WasmStr,)>(&mut store, "render")
        .map_err(|_| Refusal::Guest)?;
    let (value,) = render.call(&mut store, ()).map_err(|_| Refusal::Guest)?;
    let tentative = (|| {
        let text = value.to_str(&store).map_err(|_| Refusal::Profile)?;
        if !matches!(text, Cow::Borrowed(_)) {
            return Err(Refusal::Profile);
        }
        if text.len() > maximum {
            return Err(Refusal::OutputLimit);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(text.len())
            .map_err(|_| Refusal::Allocation)?;
        output.extend_from_slice(text.as_bytes());
        Ok(output)
    })(); // All guest-memory borrows end before post_return.
    // Mandatory even when host validation/copy refuses. Failure overrides it.
    render.post_return(&mut store).map_err(|error| {
        if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
            Refusal::PostFuel
        } else {
            Refusal::Guest
        }
    })?;
    let peek = instance
        .get_typed_func::<(), (WasmStr,)>(&mut store, "peek")
        .map_err(|_| Refusal::Guest)?;
    let (changed,) = peek.call(&mut store, ()).map_err(|_| Refusal::Guest)?;
    let observed = (|| {
        let text = changed.to_str(&store).map_err(|_| Refusal::Profile)?;
        Ok::<_, Refusal>(
            matches!(text, Cow::Borrowed(_))
                && text.len() == HTML.len()
                && text.as_bytes().first() == Some(&b'X')
                && text.as_bytes()[1..] == HTML.as_bytes()[1..],
        )
    })();
    peek.post_return(&mut store).map_err(|_| Refusal::Guest)?; // Also without post option.
    if !observed? {
        return Err(Refusal::Profile);
    }
    let bytes = tentative?;
    if bytes != HTML.as_bytes() {
        return Err(Refusal::Profile);
    }
    Ok(Rendered {
        bytes,
        post_return_completed: true,
        guest_memory_changed_after_copy: true,
    })
}
fn artifacts(maximum: usize, spinning_post: bool) -> Result<Artifacts, Refusal> {
    let rendered = render_html(maximum, spinning_post)?;
    let body_bytes = rendered.bytes.len();
    let body = String::from_utf8(rendered.bytes).map_err(|_| Refusal::Profile)?;
    let value = serde_json::json!({"format_version":1,"pages":[{
        "target":TARGET,"status":200,"content_type":"text/html; charset=utf-8","body":body}]});
    let mut payload = serde_json::to_vec(&value).map_err(|_| Refusal::Admission)?;
    payload.push(b'\n');
    if payload.len() as u64 > bundle::MAX_BUNDLE_BYTES {
        return Err(Refusal::Admission);
    }
    let receipt = bundle_receipt::receipt_from_payload(&payload).map_err(|_| Refusal::Admission)?;
    let admitted =
        bundle_receipt::verify_payload(&payload, &receipt).map_err(|_| Refusal::Admission)?;
    if admitted.targets().collect::<Vec<_>>() != [TARGET] {
        return Err(Refusal::Admission);
    }
    let summary = Summary {
        body_bytes,
        bundle_bytes: payload.len(),
        receipt_bytes: receipt.len(),
        post_return_completed: rendered.post_return_completed,
        guest_memory_changed_after_copy: rendered.guest_memory_changed_after_copy,
    };
    Ok(Artifacts {
        payload,
        receipt,
        summary,
    })
}
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file); // std provides no close-error ACK.
    Ok(())
}
fn prepare_with(
    new_dir: &Path,
    generate: impl FnOnce() -> Result<Artifacts, Refusal>,
    mut write: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> Result<Summary, Refusal> {
    match fs::symlink_metadata(new_dir) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        _ => return Err(Refusal::Io),
    }
    let parent = new_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err(Refusal::Io);
    }
    let artifacts = generate()?;
    fs::create_dir(new_dir).map_err(|_| Refusal::Io)?;
    write(&new_dir.join("bundle.json"), &artifacts.payload).map_err(|_| Refusal::Io)?;
    write(&new_dir.join("receipt.json"), &artifacts.receipt).map_err(|_| Refusal::Io)?;
    Ok(artifacts.summary)
}
/// Generate only this fixed public sample, admit one same-buffer bundle and
/// receipt, then create two new files. Partial output is retained on failure;
/// no existing entries/parents are created or overwritten. This is not atomic
/// pair publication, signed provenance, close ACK or parent durability. Parent
/// paths/host are trusted; no hostile-filesystem race protection is claimed.
pub fn prepare_fixed_publication(new_dir: &Path) -> Result<Summary, PreparationError> {
    prepare_with(new_dir, || artifacts(MAX_HTML, false), write_new)
        .map_err(|_| PreparationError(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        directory: PathBuf,
        boundary: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let boundary = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target")
                .canonicalize()
                .unwrap();
            let directory = boundary.join(format!(
                "wasm-publication-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            Self {
                directory,
                boundary,
            }
        }
        fn output(&self) -> PathBuf {
            self.directory.join("new")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let resolved = self.directory.canonicalize().unwrap();
            assert!(resolved.starts_with(&self.boundary) && resolved != self.boundary);
            fs::remove_dir_all(&resolved).unwrap();
        }
    }
    #[test]
    fn owned_html_survives_real_post_return_guest_memory_mutation() {
        let rendered = render_html(MAX_HTML, false).unwrap();
        assert_eq!(rendered.bytes, HTML.as_bytes());
        assert_eq!(HTML.len(), 153);
        assert!(!HTML.contains('\0'));
        assert!(rendered.post_return_completed && rendered.guest_memory_changed_after_copy);
    }
    #[test]
    fn cap_and_post_fuel_refusals_have_intended_priority_before_any_directory() {
        let fixture = Fixture::new();
        for (cap, spin, expected) in [
            (0, false, Refusal::OutputLimit),
            (MAX_HTML, true, Refusal::PostFuel),
            (0, true, Refusal::PostFuel),
        ] {
            assert_eq!(
                prepare_with(&fixture.output(), || artifacts(cap, spin), write_new),
                Err(expected)
            );
            assert!(!fixture.output().exists());
        }
    }
    #[test]
    fn complete_same_buffer_receipt_admits_only_the_exact_payload() {
        let artifacts = artifacts(MAX_HTML, false).unwrap();
        assert_eq!(artifacts.summary.body_bytes, HTML.len());
        assert_eq!(artifacts.summary.bundle_bytes, artifacts.payload.len());
        assert_eq!(artifacts.summary.receipt_bytes, artifacts.receipt.len());
        let site = bundle_receipt::verify_payload(&artifacts.payload, &artifacts.receipt).unwrap();
        assert_eq!(site.targets().collect::<Vec<_>>(), [TARGET]);
        let mut changed = artifacts.payload.clone();
        changed[0] = b' ';
        assert_eq!(
            bundle_receipt::verify_payload(&changed, &artifacts.receipt).unwrap_err(),
            bundle_receipt::BundleReceiptError::PayloadDigestMismatch
        );
    }
    #[test]
    fn existing_entries_and_absent_parent_fail_before_guest_generation() {
        let fixture = Fixture::new();
        let output = fixture.output();
        fs::write(&output, b"PRIVATE_CANARY").unwrap();
        assert_eq!(
            prepare_with(
                &output,
                || panic!("guest generation reached existing entry"),
                write_new
            ),
            Err(Refusal::Io)
        );
        assert_eq!(fs::read(&output).unwrap(), b"PRIVATE_CANARY");
        let missing = fixture.directory.join("absent/new");
        assert_eq!(
            prepare_with(
                &missing,
                || panic!("guest generation reached absent parent"),
                write_new
            ),
            Err(Refusal::Io)
        );
        assert!(!missing.parent().unwrap().exists());
    }
    #[test]
    fn second_write_failure_retains_partial_bundle_and_success_keeps_both_files() {
        let fixture = Fixture::new();
        let mut calls = 0;
        let result = prepare_with(
            &fixture.output(),
            || artifacts(MAX_HTML, false),
            |path, bytes| {
                calls += 1;
                if calls == 2 {
                    return Err(io::Error::other("fixed write failure"));
                }
                write_new(path, bytes)
            },
        );
        assert_eq!(result, Err(Refusal::Io));
        assert!(fixture.output().join("bundle.json").is_file());
        assert!(!fixture.output().join("receipt.json").exists());
        let success = fixture.directory.join("complete");
        let summary = prepare_fixed_publication(&success).unwrap();
        assert_eq!(
            fs::read(success.join("bundle.json")).unwrap().len(),
            summary.bundle_bytes
        );
        assert_eq!(
            fs::read(success.join("receipt.json")).unwrap().len(),
            summary.receipt_bytes
        );
        assert_eq!(fs::read_dir(&success).unwrap().count(), 2);
        assert_eq!(
            bundle_receipt::load_verified(
                &success.join("bundle.json"),
                &success.join("receipt.json")
            )
            .unwrap()
            .targets()
            .collect::<Vec<_>>(),
            [TARGET]
        );
    }
    #[test]
    fn opaque_error_diagnostics_do_not_retain_paths_or_guest_payload() {
        let error = PreparationError(());
        assert_eq!(format!("{error:?}"), "PreparationError");
        assert_eq!(error.to_string(), "Wasm publication preparation failed");
        assert!(std::error::Error::source(&error).is_none());
    }
}

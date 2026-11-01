//! A same-job generated, fixed C context diagnostic. No PHP/Fiber claim.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::{Engine, Module};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::{LimitedPipe, collect_pipe, make_engine};

const BUILD_MANIFEST: &str = "experiments/php-wasix-context-probe/manifest.json";
const BUILD_RECEIPT: &str = "experiments/php-wasix-context-probe/evidence/build.json";
const C_PATH: &str = "experiments/php-wasix-context-probe/fixtures/context_contract.c";
const MODULE_PATH: &str = "target/php-wasix-context-probe/context.wasm";
const MANIFEST: &[u8] = include_bytes!("../../php-wasix-context-probe/manifest.json");
const C_SOURCE: &[u8] = include_bytes!("../../php-wasix-context-probe/fixtures/context_contract.c");

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.wasix-context-host.v0.5");
    report["scope"] = json!("one-fixed-C-context-program-no-PHP-or-product-integration");
    report["source"] =
        json!({"kind":"same-job-fixed-C-source-build","path":C_PATH,"verified":false});
    report["artifact"] = json!({"path":MODULE_PATH,"verified":false,
        "identity_policy":"same-job-build-receipt-not-PHP-CLI-artifact-pin"});
    report["build_receipt"] = json!({"path":BUILD_RECEIPT,"verified":false,
        "full_source_tool_chain_verified_by_host":false,
        "chain_owner":"context_check.py must compare host receipt/module/source digests with its verified snapshots"});
    report["php_executed"] = json!(false);
    report["php_fiber_compatibility_verified"] = json!(false);
    report["context_contract_verified"] = json!(false);
    report["memory_limit_verified"] = json!(false);
    report["cpu_limit_verified"] = json!(false);
    report["all_resources_immediately_reclaimed"] = json!(false);
    report["sysroot_source_build_attestation_verified"] = json!(false);
    report["product_adopted"] = json!(false);
    report["isolation"]["asynchronous_threading"] = json!(true);
    report["isolation"]["deep_sleep"] = json!(false);
    report["isolation"]["guest_environment_supplied"] = json!([]);
    report["isolation"]["guest_mounts"] = json!([]);
    report["isolation"]["other_virtual_root_writes_forbidden"] = json!(false);
    report["guest_arguments"] = json!([]);
    report["module_compile_invocations"] = json!(0);
    report["guest_completed"] = json!(false);
    report["guest_json"] = Value::Null;
    report["limitations"] = json!([
        "Only one fixed C program using WASIX context create/switch/destroy is exercised; PHP and Fiber are not executed.",
        "The generated module is bound to this job's receipt/source/tool observations; this is not signed attestation or reproducible-build proof.",
        "No memory, CPU or in-process deadline limit is established; an external owned-child supervisor must enforce 60 seconds and reap the host.",
        "Runner shutdown and pipe EOF do not establish immediate reclamation of all runtime resources or a product worker.",
        "Stack ranges describe the initial simultaneous live-frame snapshot; unwound A frames are not asserted to remain live.",
        "Host directories/environment/network are not supplied; WasiRunner's other temporary virtual-root writes are not forbidden."
    ]);
}

fn read_fixed(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = root.join(name);
    ensure!(
        path.canonicalize()? == path,
        "fixed context input resolves through a symlink"
    );
    let file = File::open(&path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() > 0 && meta.len() <= limit,
        "context input size/type"
    );
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == meta.len() && bytes.len() as u64 <= limit,
        "context input changed while reading"
    );
    Ok(bytes)
}

fn load_module(report: &mut Value) -> Result<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let manifest = read_fixed(&root, BUILD_MANIFEST, 256 * 1024)?;
    let source = read_fixed(&root, C_PATH, 256 * 1024)?;
    ensure!(
        manifest == MANIFEST && source == C_SOURCE,
        "context source/manifest changed since host compilation"
    );
    let wire = read_fixed(&root, BUILD_RECEIPT, 2 * 1024 * 1024)?;
    let receipt: Value = serde_json::from_slice(&wire)?;
    report["build_receipt"]["observed_sha256"] = json!(sha(&wire));
    report["build_receipt"]["manifest_sha256"] = json!(sha(&manifest));
    report["source"]["observed_sha256"] = json!(sha(&source));
    ensure!(
        receipt["format_version"] == 1
            && receipt["outcome"] == "context_candidate_built_not_executed"
            && receipt["phase"] == "build_complete"
            && receipt["guest_executed"] == false,
        "context build did not complete without guest execution"
    );
    ensure!(
        receipt["input_files"][BUILD_MANIFEST] == sha(&manifest)
            && receipt["input_files"][C_PATH] == sha(&source),
        "context receipt source/manifest binding mismatch"
    );
    ensure!(
        receipt["context_wasm"]["path"] == MODULE_PATH
            && receipt["context_wasm"]["wasm_header_verified"] == true
            && receipt["context_wasm"]["executed"] == false,
        "unexpected context generated module identity"
    );
    let bytes = read_fixed(&root, MODULE_PATH, 16 * 1024 * 1024)?;
    ensure!(
        bytes.starts_with(b"\0asm\x01\0\0\0"),
        "context output is not WebAssembly v1"
    );
    ensure!(
        receipt["context_wasm"]["bytes"].as_u64() == Some(bytes.len() as u64)
            && receipt["context_wasm"]["sha256"] == sha(&bytes),
        "context module size/hash mismatch"
    );
    report["artifact"]["observed_sha256"] = json!(sha(&bytes));
    report["artifact"]["observed_bytes"] = json!(bytes.len());
    report["artifact"]["verified"] = json!(true);
    report["source"]["verified"] = json!(true);
    report["build_receipt"]["verified"] = json!(true);
    Ok(bytes)
}

pub fn verify(report: &mut Value) -> Result<()> {
    report["phase"] = json!("context_build_verification");
    let data = HashedModuleData::new(load_module(report)?);
    let engine = make_engine();
    report["phase"] = json!("context_engine_policy");
    report["engine_supports_async"] = json!(engine.supports_async());
    ensure!(
        engine.supports_async(),
        "context execution requires an async-capable engine"
    );
    report["phase"] = json!("module_compile");
    eprintln!("WASIX_CONTEXT_PHASE=module_compile");
    report["module_compile_invocations"] = json!(1);
    let started = Instant::now();
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile fixed C context module once")?;
    report["phase"] = json!("context_execution");
    eprintln!("WASIX_CONTEXT_PHASE=context_execution");
    execute(&engine, module, &data, report)?;
    report["phase"] = json!("context_json_verification");
    validate_guest(&report["guest_json"])?;
    report["context_contract_verified"] = json!(true);
    Ok(())
}

fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    report: &mut Value,
) -> Result<()> {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let guard = tokio_runtime.enter();
    let (stdin_sender, stdin_reader) = Pipe::channel();
    drop(stdin_sender);
    let (stdout_sender, stdout_reader) = Pipe::channel();
    let (stderr_sender, stderr_reader) = Pipe::channel();
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let outcome = (|| -> Result<()> {
        let tasks = TokioTaskManager::new(tokio_runtime.handle().clone());
        let mut runtime = PluggableRuntime::new(Arc::new(tasks));
        runtime.set_engine(engine.clone());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let mut runner = WasiRunner::new();
        runner
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(true)
            .with_stdin(Box::new(stdin_reader))
            .with_stdout(Box::new(LimitedPipe::new(
                stdout_sender,
                stdout_overflow.clone(),
            )))
            .with_stderr(Box::new(LimitedPipe::new(
                stderr_sender,
                stderr_overflow.clone(),
            )));
        let caps = runner.capabilities_mut();
        caps.insecure_allow_all = false;
        caps.threading.max_threads = Some(1);
        caps.threading.enable_deep_sleep = false;
        let started = Instant::now();
        let result = runner.run_wasm(
            RuntimeOrEngine::Runtime(Arc::new(runtime)),
            "context-probe",
            module,
            *data.hash(),
        );
        report["guest_execution_elapsed_ms"] = json!(started.elapsed().as_millis());
        drop(runner);
        result.context("execute fixed C context program")
    })();
    report["guest_completed"] = json!(outcome.is_ok());
    if let Err(error) = &outcome {
        report["host_execution_error"] =
            json!(format!("{error:#}").chars().take(4096).collect::<String>());
    }
    drop(guard);
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    report["runtime_shutdown_returned"] = json!(true);
    report["runtime_shutdown_wait_limit_ms"] = json!(2000);
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    report["stdout_eof"] = json!(stdout.is_ok());
    report["stderr_eof"] = json!(stderr.is_ok());
    report["stdout_overflow"] = json!(stdout_overflow.load(Ordering::Relaxed));
    report["stderr_overflow"] = json!(stderr_overflow.load(Ordering::Relaxed));
    if let Ok(bytes) = &stdout {
        report["stdout_bytes"] = json!(bytes.len());
        report["stdout_hex"] = json!(hex(bytes));
        report["stdout_sha256"] = json!(sha(bytes));
        match serde_json::from_slice::<Value>(bytes) {
            Ok(value) => {
                report["guest_json"] = value;
                report["guest_json_decoded"] = json!(true);
            }
            Err(error) => {
                report["guest_json_decoded"] = json!(false);
                report["guest_json_error"] = json!(error.to_string());
            }
        }
    }
    if let Ok(bytes) = &stderr {
        report["stderr_bytes"] = json!(bytes.len());
        report["stderr_hex"] = json!(hex(bytes));
    }
    // Always retain failed-guest JSON, capture and host error BEFORE returning.
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "context output exceeded limit"
    );
    let stdout = stdout.context("context stdout EOF")?;
    ensure!(
        stderr.context("context stderr EOF")?.is_empty(),
        "context guest wrote unexpected stderr"
    );
    ensure!(
        stdout.ends_with(b"\n") && stdout.iter().filter(|&&byte| byte == b'\n').count() == 1,
        "context guest must emit one terminated JSON result"
    );
    ensure!(
        report["guest_json_decoded"] == true,
        "context guest JSON could not be decoded"
    );
    Ok(())
}

fn validate_guest(value: &Value) -> Result<()> {
    let keys = [
        "schema",
        "ok",
        "phase",
        "failure",
        "failure_return",
        "failure_errno",
        "abi",
        "contexts",
        "rounds",
        "depth",
        "a_rounds",
        "b_rounds",
        "a_ready",
        "b_ready",
        "b_done",
        "a_resumed_after_destroy",
        "a_done",
        "a_unwound_frames",
        "b_unwound_frames",
        "range_observation",
        "range_count",
        "ranges",
        "trace",
        "checks",
        "errors",
    ];
    let object = value
        .as_object()
        .context("context result is not an object")?;
    ensure!(
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key)),
        "context field inventory mismatch"
    );
    ensure!(
        value["schema"] == "recastpress.wasix-context-c.v0.5" && value["ok"] == true,
        "context C diagnostic reported failure"
    );
    ensure!(
        value["phase"] == "complete",
        "context C program did not reach complete"
    );
    ensure!(
        value["failure"].is_null() && value["failure_return"] == 0 && value["failure_errno"] == 0,
        "context C diagnostic retained a syscall failure"
    );
    ensure!(
        value["abi"] == json!({"context_id_bytes":8,"pointer_bytes":4,"einval":28}),
        "context C ABI mismatch"
    );
    ensure!(
        value["rounds"] == 8
            && value["depth"] == 4
            && value["a_rounds"] == 8
            && value["b_rounds"] == 10,
        "context round/depth/continuation mismatch"
    );
    ensure!(
        value["a_ready"] == true
            && value["b_ready"] == true
            && value["b_done"] == true
            && value["a_done"] == true
            && value["a_unwound_frames"] == 4
            && value["b_unwound_frames"] == 4
            && value["range_observation"] == "initial-simultaneous-live-snapshot"
            && value["a_resumed_after_destroy"] == 0
            && value["range_count"] == 9,
        "context lifecycle mismatch"
    );
    let contexts = &value["contexts"];
    ensure!(
        contexts.as_object().is_some_and(|object| object.len() == 3),
        "context ID field inventory mismatch"
    );
    ensure!(contexts["main"] == 0, "context main ID mismatch");
    let a = contexts["a"].as_u64().context("missing context A id")?;
    let b = contexts["b"].as_u64().context("missing context B id")?;
    ensure!(a > 0 && b > 0 && a != b, "context IDs are not distinct");
    let mut expected_trace = vec![10_u64, 20];
    for round in 0..8 {
        expected_trace.extend([100 + round, 200 + round]);
    }
    expected_trace.extend([40, 208, 209, 30]);
    ensure!(
        value["trace"] == json!(expected_trace),
        "context switching trace mismatch"
    );
    let ranges = value["ranges"]
        .as_array()
        .context("missing context stack ranges")?;
    ensure!(ranges.len() == 9, "context stack range count mismatch");
    let mut seen_ranges = std::collections::BTreeSet::new();
    for (index, left) in ranges.iter().enumerate() {
        let fields = left.as_object().context("range is not an object")?;
        ensure!(
            fields.len() == 4
                && ["owner", "depth", "begin", "end"]
                    .iter()
                    .all(|key| fields.contains_key(*key)),
            "range field inventory mismatch"
        );
        let owner = left["owner"].as_str().context("missing range owner")?;
        let depth = left["depth"].as_u64().context("missing recursion depth")?;
        ensure!(
            (owner == "main" && depth == 0) || (matches!(owner, "a" | "b") && depth < 4),
            "range owner/depth mismatch"
        );
        ensure!(
            seen_ranges.insert((owner, depth)),
            "duplicate range owner/depth"
        );
        let begin = left["begin"].as_u64().context("missing range start")?;
        let end = left["end"].as_u64().context("missing range end")?;
        ensure!(
            begin < end && end <= u64::from(u32::MAX),
            "context range is not a wasm32 interval"
        );
        for right in ranges.iter().skip(index + 1) {
            let other_begin = right["begin"].as_u64().context("missing range start")?;
            let other_end = right["end"].as_u64().context("missing range end")?;
            ensure!(
                end <= other_begin || other_end <= begin,
                "context linear-stack ranges overlap"
            );
        }
    }
    let check_names = [
        "main_self_switch",
        "a_self_switch",
        "destroy_main_rejected",
        "destroy_self_a_rejected",
        "destroy_main_from_a_rejected",
        "ranges_disjoint",
        "canaries_preserved",
        "trace_matches",
        "destroy_a_succeeded",
        "switch_destroyed_a_rejected",
        "destroy_a_twice_succeeded",
        "b_continued_after_a_destroy",
        "b_recursion_unwound",
        "destroy_b_succeeded",
        "switch_destroyed_b_rejected",
        "destroy_b_twice_succeeded",
        "a_recursion_unwound",
        "main_clobber_after_a_unwind",
        "b_preserved_after_a_unwind",
    ];
    let expected_checks = check_names
        .into_iter()
        .map(|name| (name.to_owned(), json!(true)))
        .collect::<serde_json::Map<_, _>>();
    ensure!(
        value["checks"] == Value::Object(expected_checks),
        "context checks did not all pass"
    );
    let error_names = [
        "destroy_main_from_main",
        "destroy_self_a",
        "destroy_main_from_a",
        "switch_destroyed_a",
        "switch_destroyed_b",
    ];
    let expected_errors = error_names
        .into_iter()
        .map(|name| (name.to_owned(), json!({"return":-1,"errno":28})))
        .collect::<serde_json::Map<_, _>>();
    ensure!(
        value["errors"] == Value::Object(expected_errors),
        "context rejected-operation result/errno mismatch"
    );
    Ok(())
}

//! A same-job generated, fixed C context diagnostic. No PHP/Fiber claim.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::wasmparser::{ExternalKind, Operator, Parser, Payload, TypeRef, ValType};
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
const MODULE_PATH: &str = "target/php-wasix-context-probe/context.managed.wasm";
const MANIFEST: &[u8] = include_bytes!("../../php-wasix-context-probe/manifest.json");
const C_SOURCE: &[u8] = include_bytes!("../../php-wasix-context-probe/fixtures/context_contract.c");
const ADAPTER_SOURCES: [(&str, &[u8]); 3] = [
    (
        "experiments/php-wasix-context-probe/fixtures/stack_adapter.c",
        include_bytes!("../../php-wasix-context-probe/fixtures/stack_adapter.c"),
    ),
    (
        "experiments/php-wasix-context-probe/fixtures/stack_adapter.h",
        include_bytes!("../../php-wasix-context-probe/fixtures/stack_adapter.h"),
    ),
    (
        "experiments/php-wasix-context-probe/fixtures/stack_adapter.S",
        include_bytes!("../../php-wasix-context-probe/fixtures/stack_adapter.S"),
    ),
];

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
    report["variant"] = json!("managed-guest-linear-stacks-v1");
    report["stack_adapter"] = json!({"slots":2,"stack_bytes_per_context":65536,"alignment_bytes":16,
        "buffers_freed":false,"buffers_reused":false,"allocation_lifetime":"owned guest process exit",
        "source_files":[],"postopt_verified":false});
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
        "Two fixed 64KiB guest heap stacks are retained until process exit; no guards, free/reuse safety, PHP Fiber adapter or cancellation recovery is verified.",
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
    let definition: Value = serde_json::from_slice(&manifest)?;
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
        receipt["variant"] == "managed-guest-linear-stacks-v1"
            && receipt["stack_adapter"] == definition["stack_adapter"],
        "managed adapter configuration mismatch"
    );
    for (name, embedded) in ADAPTER_SOURCES {
        let bytes = read_fixed(&root, name, 256 * 1024)?;
        ensure!(
            bytes == embedded
                && receipt["input_files"][name] == sha(&bytes)
                && receipt["adapter_sources"][name]["sha256"] == sha(&bytes)
                && receipt["adapter_sources"][name]["bytes"].as_u64() == Some(bytes.len() as u64),
            "context adapter source changed since host/build compilation"
        );
        report["stack_adapter"]["source_files"]
            .as_array_mut()
            .context("adapter sources not initialized")?
            .push(json!({"path":name,"bytes":bytes.len(),"sha256":sha(&bytes),"verified":true}));
    }
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

/// Inspect the final bytes that will be compiled, after Binaryen. Exact closed
/// bodies reject hidden C prologues/spills, extra calls and target-SP prechanges.
/// This is structural verification, not proof of compiler or runtime correctness.
fn inspect_adapter(bytes: &[u8]) -> Result<Value> {
    let mut types = Vec::new();
    let mut function_types = Vec::new();
    let mut imports = Vec::new();
    let mut globals = Vec::new();
    let mut exports = std::collections::BTreeMap::new();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    types.push(ty?);
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import?;
                    match import.ty {
                        TypeRef::Func(ty) | TypeRef::FuncExact(ty) => {
                            function_types.push(ty);
                            imports.push((import.module, import.name));
                        }
                        TypeRef::Global(ty) => globals.push(ty),
                        _ => {}
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    function_types.push(ty?);
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    globals.push(global?.ty);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export?;
                    ensure!(
                        exports
                            .insert(export.name, (export.kind, export.index))
                            .is_none(),
                        "duplicate context export"
                    );
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    ensure!(
        bodies.len() + imports.len() == function_types.len(),
        "context function/code count mismatch"
    );
    let exported_function = |name: &str, params: &[ValType], results: &[ValType]| -> Result<u32> {
        let &(kind, index) = exports
            .get(name)
            .context("missing managed adapter function export")?;
        ensure!(
            kind == ExternalKind::Func && index as usize >= imports.len(),
            "adapter export is not a defined function"
        );
        let &ty = function_types
            .get(index as usize)
            .context("adapter function index missing")?;
        let ty = types
            .get(ty as usize)
            .context("adapter type index missing")?;
        ensure!(
            ty.params() == params && ty.results() == results,
            "adapter function signature mismatch"
        );
        Ok(index)
    };
    let switch = exported_function("rp_stack_switch_raw", &[ValType::I64], &[ValType::I32])?;
    let entry_a = exported_function("rp_stack_entry_a", &[], &[])?;
    let entry_b = exported_function("rp_stack_entry_b", &[], &[])?;
    let dispatch_a = exported_function("rp_stack_dispatch_a", &[], &[])?;
    let dispatch_b = exported_function("rp_stack_dispatch_b", &[], &[])?;
    ensure!(
        [switch, entry_a, entry_b, dispatch_a, dispatch_b]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == 5,
        "adapter exported functions alias each other"
    );
    let &(kind, sp) = exports
        .get("__stack_pointer")
        .context("missing explicit stack pointer export")?;
    ensure!(
        kind == ExternalKind::Global,
        "stack pointer export is not a global"
    );
    let global = globals
        .get(sp as usize)
        .context("stack pointer global missing")?;
    ensure!(
        global.mutable && global.content_type == ValType::I32,
        "stack pointer must be mutable i32"
    );
    let read_body = |index: u32| -> Result<_> {
        let body = bodies
            .get(index as usize - imports.len())
            .context("adapter body missing")?;
        let locals = body
            .get_locals_reader()?
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            locals.len() <= 8
                && locals
                    .iter()
                    .map(|(count, _)| u64::from(*count))
                    .sum::<u64>()
                    <= 8,
            "unexpected adapter locals"
        );
        let mut reader = body.get_operators_reader()?;
        let mut operators = Vec::new();
        while !reader.eof() {
            ensure!(
                operators.len() < 32,
                "adapter body is not the fixed small assembly"
            );
            operators.push(reader.read()?);
        }
        Ok((locals, operators))
    };
    let (locals, ops) = read_body(switch)?;
    let (raw, switch_sequence, local_count) = match ops.as_slice() {
        [
            Operator::GlobalGet { global_index: get },
            Operator::LocalSet { local_index: 1 },
            Operator::LocalGet { local_index: 0 },
            Operator::Call {
                function_index: raw,
            },
            Operator::LocalGet { local_index: 1 },
            Operator::GlobalSet { global_index: set },
            Operator::End,
        ] if *get == sp && *set == sp && locals == [(1, ValType::I32)] => (
            *raw,
            vec![
                "global.get",
                "local.set",
                "local.get",
                "call",
                "local.get",
                "global.set",
                "end",
            ],
            1,
        ),
        [
            Operator::GlobalGet { global_index: get },
            Operator::LocalSet { local_index: 1 },
            Operator::LocalGet { local_index: 0 },
            Operator::Call {
                function_index: raw,
            },
            Operator::LocalSet { local_index: 2 },
            Operator::LocalGet { local_index: 1 },
            Operator::GlobalSet { global_index: set },
            Operator::LocalGet { local_index: 2 },
            Operator::End,
        ] if *get == sp
            && *set == sp
            && (locals == [(2, ValType::I32)]
                || locals == [(1, ValType::I32), (1, ValType::I32)]) =>
        {
            (
                *raw,
                vec![
                    "global.get",
                    "local.set",
                    "local.get",
                    "call",
                    "local.set",
                    "local.get",
                    "global.set",
                    "local.get",
                    "end",
                ],
                2,
            )
        }
        _ => bail!(
            "postopt switch does not save SP, call raw import, and immediately restore without stack memory operations"
        ),
    };
    ensure!(
        imports.get(raw as usize) == Some(&("wasix_32v1", "context_switch")),
        "switch calls the wrong import"
    );
    let raw_type = types
        .get(
            *function_types
                .get(raw as usize)
                .context("raw switch type missing")? as usize,
        )
        .context("raw switch signature missing")?;
    ensure!(
        raw_type.params() == [ValType::I64] && raw_type.results() == [ValType::I32],
        "raw switch ABI mismatch"
    );
    let mut addresses = Vec::new();
    for (entry, dispatch) in [(entry_a, dispatch_a), (entry_b, dispatch_b)] {
        let (locals, ops) = read_body(entry)?;
        ensure!(
            locals.is_empty(),
            "entry trampoline has unexpected locals/prologue"
        );
        let address = match ops.as_slice() {
            [
                Operator::I32Const { value },
                Operator::I32Load { memarg },
                Operator::GlobalSet { global_index },
                Operator::Call { function_index },
                Operator::Unreachable,
                Operator::End,
            ] if *value >= 0
                && memarg.memory == 0
                && memarg.align <= 2
                && *global_index == sp
                && *function_index == dispatch =>
            {
                u64::from(*value as u32)
                    .checked_add(memarg.offset)
                    .context("adapter metadata address overflow")?
            }
            _ => bail!(
                "postopt entry must load fixed metadata, set SP before C dispatch, and trap on return"
            ),
        };
        ensure!(
            address > 0 && address <= u64::from(u32::MAX) && address % 4 == 0,
            "adapter metadata is not an aligned wasm32 word"
        );
        addresses.push(address);
    }
    ensure!(
        addresses[0] != addresses[1],
        "entry trampolines share one metadata address"
    );
    Ok(
        json!({"verified":true,"parser":"wasmer::wasmparser","inspected_bytes_sha256":sha(bytes),
        "scope":"exact closed final-Wasm assembly shapes; not a compiler/runtime/native-stack correctness proof",
        "function_exports":["rp_stack_switch_raw","rp_stack_entry_a","rp_stack_entry_b","rp_stack_dispatch_a","rp_stack_dispatch_b"],
        "stack_global_export":"__stack_pointer","stack_global_index":sp,"stack_global_type":"mutable i32",
        "switch_import":{"module":"wasix_32v1","name":"context_switch","parameters":["i64"],"results":["i32"]},
        "switch_operator_sequence":switch_sequence,"switch_wasm_local_count":local_count,
        "entry_operator_sequence":["i32.const","i32.load","global.set","call","unreachable","end"],
        "entry_metadata_addresses":addresses,"linear_stack_memory_writes_in_shims":0,
        "target_stack_pointer_changed_before_switch":false,"restores_before_return_to_C":true,
        "sets_stack_before_initial_C_dispatch":true}),
    )
}

pub fn verify(report: &mut Value) -> Result<()> {
    report["phase"] = json!("context_build_verification");
    let bytes = load_module(report)?;
    report["phase"] = json!("context_adapter_verification");
    report["stack_adapter"]["postopt"] = inspect_adapter(&bytes)?;
    report["stack_adapter"]["postopt_verified"] = json!(true);
    let data = HashedModuleData::new(bytes);
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

#[cfg(test)]
mod tests {
    use super::inspect_adapter;

    /// Parser-only: no Engine, Module, instance or guest execution is used.
    #[test]
    fn final_adapter_shapes_accept_register_only_switches_and_reject_unsafe_mutants() {
        let template = r#"
        (module
          (import "wasix_32v1" "context_switch" (func $raw (param i64) (result i32)))
          (memory 1)
          (global $sp (export "__stack_pointer") (mut i32) (i32.const 65536))
          (global $other (mut i32) (i32.const 32768))
          (func $extra)
          (func $lookalike (param i64) (result i32) i32.const 0)
          (func $dispatch_a (export "rp_stack_dispatch_a"))
          (func $dispatch_b (export "rp_stack_dispatch_b"))
          (func $entry_a (export "rp_stack_entry_a")
            i32.const 4 i32.load global.set $sp
            call $dispatch_a unreachable)
          (func $entry_b (export "rp_stack_entry_b")
            i32.const 8 i32.load global.set $sp
            call $dispatch_b unreachable)
          (func $switch (export "rp_stack_switch_raw") (param i64) (result i32)
            SWITCH_BODY))
        "#;
        let variants = [
            (
                1,
                "(local i32) global.get $sp local.set 1 local.get 0 call $raw local.get 1 global.set $sp",
            ),
            (
                2,
                "(local i32 i32) global.get $sp local.set 1 local.get 0 call $raw local.set 2 local.get 1 global.set $sp local.get 2",
            ),
        ];
        for (locals, body) in variants {
            let source = template.replace("SWITCH_BODY", body);
            let accepted = inspect_adapter(&wat::parse_str(&source).unwrap()).unwrap();
            assert_eq!(accepted["verified"], true);
            assert_eq!(accepted["switch_wasm_local_count"], locals);
            assert_eq!(
                accepted["entry_metadata_addresses"],
                serde_json::json!([4, 8])
            );
            let mutants = [
                (
                    "linear stack store",
                    source.replace(
                        "local.set 1 local.get 0",
                        "local.set 1 i32.const 16 i32.const 99 i32.store local.get 0",
                    ),
                ),
                (
                    "extra call",
                    source.replace("call $raw", "call $raw call $extra"),
                ),
                (
                    "SP changed before switch",
                    source.replace(
                        "local.set 1 local.get 0",
                        "local.set 1 i32.const 1234 global.set $sp local.get 0",
                    ),
                ),
                (
                    "wrong import name",
                    source.replace("\"context_switch\"", "\"context_create\""),
                ),
                (
                    "wrong import module",
                    source.replace("\"wasix_32v1\"", "\"wasi_snapshot_preview1\""),
                ),
                (
                    "wrong raw import type",
                    source.replace("(func $raw (param i64)", "(func $raw (param i32)"),
                ),
                (
                    "wrong switch export type",
                    source.replace(
                        "(export \"rp_stack_switch_raw\") (param i64)",
                        "(export \"rp_stack_switch_raw\") (param i32)",
                    ),
                ),
                (
                    "defined wrapper instead of raw import",
                    source.replace("call $raw", "call $lookalike"),
                ),
                (
                    "wrong actual SP global",
                    source
                        .replace("global.get $sp", "global.get $other")
                        .replace("global.set $sp", "global.set $other"),
                ),
                (
                    "immutable SP export",
                    source.replace(
                        "(export \"__stack_pointer\") (mut i32)",
                        "(export \"__stack_pointer\") i32",
                    ),
                ),
                (
                    "duplicate entry metadata",
                    source.replace("i32.const 8 i32.load", "i32.const 4 i32.load"),
                ),
                (
                    "wrong entry dispatch",
                    source.replace(
                        "call $dispatch_b unreachable",
                        "call $dispatch_a unreachable",
                    ),
                ),
                (
                    "C dispatch before SP set",
                    source.replace(
                        "i32.const 4 i32.load global.set $sp\n            call $dispatch_a",
                        "i32.const 4 i32.load call $dispatch_a\n            global.set $sp",
                    ),
                ),
                (
                    "entry linear memory store",
                    source.replace(
                        "i32.const 4 i32.load",
                        "i32.const 16 i32.const 99 i32.store i32.const 4 i32.load",
                    ),
                ),
            ];
            for (reason, mutant) in mutants {
                assert_ne!(mutant, source, "mutation was not applied: {reason}");
                // Some wrong-type mutants are intentionally invalid Wasm. The
                // structural guard must reject them before Module compilation.
                let bytes = wat::parse_str(&mutant).unwrap();
                assert!(
                    inspect_adapter(&bytes).is_err(),
                    "accepted {locals}-local mutant: {reason}"
                );
            }
            if locals == 2 {
                let alias = source
                    .replace("local.set 2", "local.set 1")
                    .replace("local.get 2", "local.get 1");
                assert!(
                    inspect_adapter(&wat::parse_str(&alias).unwrap()).is_err(),
                    "errno overwrote saved SP"
                );
            }
        }
    }
}

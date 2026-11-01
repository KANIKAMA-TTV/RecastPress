//! Same-job opt-in PHP Fiber build. A fixed corpus, never a product runtime.

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

const BASE: &str = "experiments/php-wasix-fiber-probe";
const MODULE: &str = "target/php-wasix-fiber-probe/php-fiber.wasm";
const RECEIPT: &str = "experiments/php-wasix-fiber-probe/evidence/build.json";
const SOURCES: [(&str, &[u8]); 5] = [
    (
        "manifest.json",
        include_bytes!("../../php-wasix-fiber-probe/manifest.json"),
    ),
    (
        "patches/0001-nts-fresh-wasi-zend-fibers.patch",
        include_bytes!("../../php-wasix-fiber-probe/patches/0001-nts-fresh-wasi-zend-fibers.patch"),
    ),
    (
        "fixtures/zend_wasix_fiber.S",
        include_bytes!("../../php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S"),
    ),
    (
        "fixtures/fiber_contract.php",
        include_bytes!("../../php-wasix-fiber-probe/fixtures/fiber_contract.php"),
    ),
    (
        "evidence/native-fiber-contract.json",
        include_bytes!("../../php-wasix-fiber-probe/evidence/native-fiber-contract.json"),
    ),
];
const FIXTURE: &str = include_str!("../../php-wasix-fiber-probe/fixtures/fiber_contract.php");
const NATIVE: &[u8] =
    include_bytes!("../../php-wasix-fiber-probe/evidence/native-fiber-contract.json");

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_fixed(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = root.join(name);
    ensure!(
        path.canonicalize()? == path,
        "fiber input resolves through a symlink"
    );
    let file = File::open(&path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() > 0 && meta.len() <= limit,
        "fiber input size/type"
    );
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == meta.len() && bytes.len() as u64 <= limit,
        "fiber input changed while reading"
    );
    Ok(bytes)
}

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-fiber-host.v0.5");
    report["scope"] = json!("same-job-opt-in-NTS-fresh-WASI-PHP-fixed-Fiber-corpus");
    report["source"] = json!({"repository":"wasix-org/php", "commit":"f4bfae79d5c19028349082f707930cf0951d23b9",
        "kind":"same-job-patched-source-build", "verified":false});
    report["artifact"] = json!({"path":MODULE,"verified":false,"identity_policy":"same-job-build-not-official-CLI-pin"});
    report["build_receipt"] = json!({"path":RECEIPT,"verified":false,"full_source_tool_chain_verified_by_host":false,
        "chain_owner":"fiber_check.py independently verifies builder source/tool/module chain"});
    report["variant"] = json!("nts-fresh-wasi-zend-fibers-v1");
    report["php_fiber_compatibility_verified_for_fixed_cases"] = json!(false);
    report["full_php_compatibility_verified"] = json!(false);
    report["product_runtime_selected"] = json!(false);
    report["native_runtime_fallback_used"] = json!(false);
    report["memory_limit_verified"] = json!(false);
    report["cpu_limit_verified"] = json!(false);
    report["all_resources_immediately_reclaimed"] = json!(false);
    report["sysroot_source_build_attestation_verified"] = json!(false);
    report["physical_stack_free_verified"] = json!(false);
    report["physical_stack_reuse_verified"] = json!(false);
    report["stack_guards_verified"] = json!(false);
    report["persistent_requests_supported"] = json!(false);
    report["zts_supported"] = json!(false);
    report["isolation"] = json!({"asynchronous_threading":true,"deep_sleep":false,"max_guest_threads":1,
        "host_environment_forwarded":false,"home_mapped":false,"guest_environment_supplied":[],
        "host_directories_mapped":false,"guest_mounts":[],"networking":"UnsupportedVirtualNetworking",
        "http_client":false,"stdin":"closed-pipe","stdout_limit_bytes":65536,"stderr_limit_bytes":65536,
        "other_virtual_root_writes_forbidden":false});
    report["module_compile_invocations"] = json!(0);
    report["cases"] = json!([]);
    report["limitations"] = json!([
        "Only the fixed native six-case corpus, three fresh WASI instances, NTS and one thread are exercised.",
        "No product WordPress, DB, cache, memory/CPU limit, persistent worker, ZTS or arbitrary plugin compatibility is established.",
        "Quarantined C stacks/descriptors survive PHP request cleanup; no guest free/reuse or immediate native-context reclamation is proven.",
        "Pipe EOF, bounded runtime shutdown and owned-child reap do not prove aggregate resource reclamation or cancellation safety.",
        "External owned-child supervisor must bound compile plus execution; a timeout is a failure, never Fiber success.",
        "One explicit 2097152-byte stack setting is compared; default/invalid INI, heap OOM and physical stack exhaustion remain separate gates."
    ]);
}

fn load_module(report: &mut Value) -> Result<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let wire = read_fixed(&root, RECEIPT, 2 * 1024 * 1024)?;
    let receipt: Value = serde_json::from_slice(&wire)?;
    ensure!(
        receipt["schema"] == "recastpress.php-wasix-fiber-build.v0.5"
            && receipt["outcome"] == "fiber_candidate_built_not_executed"
            && receipt["phase"] == "build_complete"
            && receipt["variant"] == "nts-fresh-wasi-zend-fibers-v1"
            && receipt["guest_executed"] == false,
        "fiber build did not complete without execution"
    );
    report["source_files"] = json!([]);
    for (relative, embedded) in SOURCES {
        let name = format!("{BASE}/{relative}");
        let bytes = read_fixed(&root, &name, 256 * 1024)?;
        ensure!(
            bytes == embedded,
            "fiber source/oracle changed since host compilation"
        );
        // The native oracle is not an input to the PHP compiler. It is bound
        // separately by the host and the external semantic checker.
        if relative != "evidence/native-fiber-contract.json" {
            ensure!(
                receipt["input_files"][&name] == sha(&bytes),
                "fiber builder source binding mismatch"
            );
        }
        report["source_files"]
            .as_array_mut()
            .context("source files")?
            .push(json!({"path":name,"bytes":bytes.len(),"sha256":sha(&bytes),"verified":true}));
    }
    ensure!(
        receipt["php_wasm"]["path"] == MODULE
            && receipt["php_wasm"]["wasm_header_verified"] == true
            && receipt["php_wasm"]["executed"] == false,
        "unexpected fiber artifact path/type"
    );
    let bytes = read_fixed(&root, MODULE, 128 * 1024 * 1024)?;
    ensure!(
        bytes.starts_with(b"\0asm\x01\0\0\0"),
        "fiber artifact is not Wasm v1"
    );
    ensure!(
        receipt["php_wasm"]["bytes"].as_u64() == Some(bytes.len() as u64)
            && receipt["php_wasm"]["sha256"] == sha(&bytes),
        "fiber artifact digest mismatch"
    );
    report["build_receipt"]["observed_sha256"] = json!(sha(&wire));
    report["build_receipt"]["verified"] = json!(true);
    report["source"]["verified"] = json!(true);
    report["artifact"]["observed_bytes"] = json!(bytes.len());
    report["artifact"]["observed_sha256"] = json!(sha(&bytes));
    report["artifact"]["verified"] = json!(true);
    Ok(bytes)
}

pub fn verify(report: &mut Value) -> Result<()> {
    report["phase"] = json!("fiber_build_verification");
    let bytes = load_module(report)?;
    report["phase"] = json!("fiber_adapter_verification");
    report["postopt_adapter"] = super::fiber_adapter::inspect(&bytes)?;
    let data = HashedModuleData::new(bytes);
    let engine = make_engine();
    report["engine_supports_async"] = json!(engine.supports_async());
    ensure!(
        engine.supports_async(),
        "Fiber requires async-capable engine"
    );
    report["phase"] = json!("module_compile");
    eprintln!("PHP_FIBER_PHASE=module_compile");
    report["module_compile_invocations"] = json!(1);
    let started = Instant::now();
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile patched PHP once")?;
    let native: Value = serde_json::from_slice(NATIVE)?;
    ensure!(
        native["outcome"] == "native_fiber_contract_passed"
            && native["native_contract_passed"] == true,
        "native Fiber oracle is not successful"
    );
    let expected = &native["observation"];
    ensure!(
        expected["ok"] == true && expected["cases"].as_object().is_some_and(|c| c.len() == 6),
        "native oracle six cases"
    );
    report["native_oracle"] =
        json!({"sha256":sha(NATIVE),"expected_case_count":6,"expected_values_weakened":false});
    report["phase"] = json!("fiber_execution");
    let mut passed = true;
    for (index, name) in ["normal_first", "normal_repeat", "normal_final"]
        .into_iter()
        .enumerate()
    {
        let mut case = json!({"name":name,"request_number":index+1,"module_delivery":"Module.clone",
            "module_compile_invocations":0,"fresh_wasi_runtime":true,"ok":false,"native_semantics_match":false,
            "guest_json":null,"guest_json_decoded":false,"guest_arguments":["-n","-d","fiber.stack_size=2097152","-r","<fixed-fiber-contract>"],
            "program_sha256":sha(FIXTURE.strip_prefix("<?php").context("fixed PHP tag")?.as_bytes())});
        let result = execute(&engine, module.clone(), &data, &mut case).and_then(|()| {
            ensure!(
                case["guest_json"] == *expected,
                "fixed Fiber result differs from native oracle"
            );
            case["native_semantics_match"] = json!(true);
            Ok(())
        });
        case["expectation_met"] = json!(result.is_ok());
        if let Err(error) = result {
            passed = false;
            case["expectation_error"] =
                json!(format!("{error:#}").chars().take(4096).collect::<String>());
        }
        report["cases"].as_array_mut().context("cases")?.push(case);
    }
    ensure!(passed, "patched PHP Fiber native comparison failed");
    report["php_fiber_compatibility_verified_for_fixed_cases"] = json!(true);
    report["exceptions_executed"] = json!(true);
    report["exception_support"]["execution_verdict"] = json!("fixed-fiber-corpus-passed");
    report["integer_width_matches_native"] = json!(true);
    Ok(())
}

fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    record: &mut Value,
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
            .with_args([
                "-n",
                "-d",
                "fiber.stack_size=2097152",
                "-r",
                FIXTURE.strip_prefix("<?php").context("fixed PHP tag")?,
            ])
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
            "php-fiber",
            module,
            *data.hash(),
        );
        record["guest_execution_elapsed_ms"] = json!(started.elapsed().as_millis());
        drop(runner);
        result.context("run fixed patched PHP Fiber contract")
    })();
    record["ok"] = json!(outcome.is_ok());
    if let Err(error) = &outcome {
        record["host_execution_error"] =
            json!(format!("{error:#}").chars().take(4096).collect::<String>());
    }
    drop(guard);
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    record["runtime_shutdown_returned"] = json!(true);
    record["runtime_shutdown_wait_limit_ms"] = json!(2000);
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    record["stdout_eof"] = json!(stdout.is_ok());
    record["stderr_eof"] = json!(stderr.is_ok());
    record["stdout_overflow"] = json!(stdout_overflow.load(Ordering::Relaxed));
    record["stderr_overflow"] = json!(stderr_overflow.load(Ordering::Relaxed));
    if let Ok(bytes) = &stdout {
        record["stdout_bytes"] = json!(bytes.len());
        record["stdout_hex"] = json!(hex(bytes));
        record["stdout_sha256"] = json!(sha(bytes));
        match serde_json::from_slice::<Value>(bytes) {
            Ok(value) => {
                record["guest_json"] = value;
                record["guest_json_decoded"] = json!(true);
            }
            Err(error) => {
                record["guest_json_error"] = json!(error.to_string());
            }
        }
    }
    if let Ok(bytes) = &stderr {
        record["stderr_bytes"] = json!(bytes.len());
        record["stderr_hex"] = json!(hex(bytes));
    }
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "Fiber output exceeded quota"
    );
    ensure!(
        stderr.context("Fiber stderr EOF")?.is_empty(),
        "Fiber unexpected stderr"
    );
    let stdout = stdout.context("Fiber stdout EOF")?;
    ensure!(
        stdout.ends_with(b"\n") && stdout.iter().filter(|&&b| b == b'\n').count() == 1,
        "Fiber JSON must be one terminated line"
    );
    ensure!(
        record["guest_json_decoded"] == true,
        "Fiber JSON could not be decoded"
    );
    Ok(())
}

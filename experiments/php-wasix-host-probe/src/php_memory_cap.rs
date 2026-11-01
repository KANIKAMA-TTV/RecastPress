//! Fixed PHP diagnostics only; 256MiB is an experiment value, not a product setting.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use wasmer::{Engine, Module, Pages};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::memory_policy::{MemoryPolicyManager, MemorySnapshot, snapshot};
use super::{LimitedPipe, OUTPUT_LIMIT, collect_pipe, make_engine};

// The stock artifact allocates a 128MiB OPcache segment before script startup.
// A 64MiB trial prevented even the normal control from reaching PHP code.
const CAP: Pages = Pages(4096);
const ALLOCATION_BYTES: u64 = 128 * 1024 * 1024;
const READY: &str = "RP_PHP_MEMORY_ALLOCATION_READY memory_limit=-1\n";
const ECHO: &str = "recastpress-v0.5-php-memory-cap";
const NORMAL: &str = r#"
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
'kind'=>'normal','memory_limit'=>ini_get('memory_limit'),'allocated_bytes'=>0,
'first_byte'=>null,'last_byte'=>null,'echo'=>'recastpress-v0.5-php-memory-cap'], JSON_THROW_ON_ERROR);
"#;
const ALLOCATE: &str = r#"
fwrite(STDERR, 'RP_PHP_MEMORY_ALLOCATION_READY memory_limit='.ini_get('memory_limit')."\n");
$payload = str_repeat('x', 128 * 1024 * 1024);
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
'kind'=>'allocation','memory_limit'=>ini_get('memory_limit'),'allocated_bytes'=>strlen($payload),
'first_byte'=>ord($payload[0]),'last_byte'=>ord($payload[strlen($payload)-1]),
'echo'=>'recastpress-v0.5-php-memory-cap'], JSON_THROW_ON_ERROR);
"#;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    capped: bool,
    allocate: bool,
    success_expected: bool,
}
const CASES: [Case; 4] = [
    Case {
        name: "capped_normal",
        capped: true,
        allocate: false,
        success_expected: true,
    },
    Case {
        name: "capped_allocation_oom",
        capped: true,
        allocate: true,
        success_expected: false,
    },
    Case {
        name: "capped_normal_recovery",
        capped: true,
        allocate: false,
        success_expected: true,
    },
    Case {
        name: "uncapped_allocation_control",
        capped: false,
        allocate: true,
        success_expected: true,
    },
];

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-memory-cap.v0.5");
    report["scope"] = json!("fixed-four-case-php-memory-cap-diagnostic-not-product-setting");
    report["diagnostic_verified"] = json!(false);
    report["php_linear_memory_cap_verified_for_fixed_cases"] = json!(false);
    report["production_memory_limit_verified"] = json!(false);
    report["rss_limit_verified"] = json!(false);
    report["compiler_memory_limit_verified"] = json!(false);
    report["aggregate_memory_limit_verified"] = json!(false);
    report["native_comparison_performed"] = json!(false);
    report["product_adopted"] = json!(false);
    report["test_cap_pages"] = json!(CAP.0);
    report["test_cap_bytes"] = json!(u64::from(CAP.0) * 65536);
    report["allocation_bytes"] = json!(ALLOCATION_BYTES);
    report["isolation"]["guest_environment_supplied"] = json!([]);
    report["isolation"]["guest_fixture_files"] = json!([]);
    report["reuse"] = json!({"php_module_compile_invocations":0,"module_clone_count":0,
        "request_path_module_compile_calls":0,"observed_cases":0,
        "counter_scope":"explicit PHP Module::new call site, not internal backend instrumentation"});
    report["cases"] = json!([]);
    report["verdict_scope"] =
        json!("Top-level ok describes the diagnostic; the expected OOM case remains ok=false.");
    report["limitations"] = json!([
        "256MiB bounds one imported shared linear memory in fixed diagnostic cases; no product limit is selected.",
        "RSS, compiler allocations, aggregate memory, CPU and in-process deadlines remain unverified.",
        "Only four fixed PHP cases are exercised; WordPress, database and full PHP compatibility remain unverified.",
        "Wasmer 7.2.0 MSVC exception runtime entry points remain unimplemented."
    ]);
}

pub fn verify(bytes: Vec<u8>, report: &mut Value) -> Result<()> {
    let data = HashedModuleData::new(bytes);
    let engine = make_engine();
    let identity = format!("{:?}", engine.id());
    report["reuse"]["configured_engine_identity"] = json!(identity);
    report["phase"] = json!("module_compile");
    eprintln!("PHP_WASIX_PHASE=module_compile");
    report["reuse"]["php_module_compile_invocations"] = json!(1);
    let started = Instant::now();
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile fixed PHP module once for memory diagnostic")?;
    let mut all_met = true;
    let mut width = None;
    for (index, case) in CASES.iter().enumerate() {
        report["phase"] = json!(case.name);
        eprintln!("PHP_WASIX_PHASE={}", case.name);
        report["reuse"]["module_clone_count"] = json!(index + 1);
        let mut record = json!({"name":case.name,"configured_engine_identity":identity,
            "success_expected":case.success_expected,"ok":false,"expectation_met":false,
            "ok_scope":"WasiRunner execution result, not the diagnostic verdict",
            "cap_pages":if case.capped { Some(CAP.0) } else { None },
            "module_delivery":"Module.clone","module_compile_invocations":0,
            "guest_environment_supplied":[],"host_environment_forwarded":false,
            "host_directories_mapped":false,"home_mapped":false,
            "networking":"UnsupportedVirtualNetworking","http_client":false,
            "stdin":"closed-pipe","stdout_limit_bytes":OUTPUT_LIMIT,"stderr_limit_bytes":OUTPUT_LIMIT,
            "oom_observed":false});
        let result = execute(&engine, module.clone(), &data, *case, &mut record)
            .and_then(|()| validate(&record, *case))
            .and_then(|observed_width| {
                if let Some(observed) = observed_width {
                    if let Some(previous) = width {
                        ensure!(
                            previous == observed,
                            "PHP integer width changed between cases"
                        );
                    }
                    width = Some(observed);
                }
                Ok(())
            });
        record["expectation_met"] = json!(result.is_ok());
        if let Err(error) = result {
            all_met = false;
            record["expectation_error"] =
                json!(format!("{error:#}").chars().take(4096).collect::<String>());
        }
        report["cases"]
            .as_array_mut()
            .expect("describe initialized cases")
            .push(record);
        report["reuse"]["observed_cases"] = json!(index + 1);
    }
    ensure!(
        all_met,
        "the fixed four-case PHP memory diagnostic did not meet every expectation"
    );
    report["observed_php_int_size"] = json!(width);
    report["diagnostic_verified"] = json!(true);
    report["php_linear_memory_cap_verified_for_fixed_cases"] = json!(true);
    Ok(())
}

fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    case: Case,
    record: &mut Value,
) -> Result<()> {
    let memories = Arc::new(Mutex::new(Vec::<MemorySnapshot>::new()));
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
        let observed = memories.clone();
        let manager = MemoryPolicyManager::new(
            TokioTaskManager::new(tokio_runtime.handle().clone()),
            if case.capped { Some(CAP) } else { None },
            move |props| {
                observed
                    .lock()
                    .expect("memory observations poisoned")
                    .push(snapshot(props))
            },
        );
        let mut runtime = PluggableRuntime::new(Arc::new(manager));
        runtime.set_engine(engine.clone());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let mut runner = WasiRunner::new();
        runner
            .with_args([
                "-n",
                "-d",
                "memory_limit=-1",
                "-d",
                "display_errors=stderr",
                "-d",
                "log_errors=0",
                "-r",
                if case.allocate { ALLOCATE } else { NORMAL },
            ])
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(false)
            .with_stdin(Box::new(stdin_reader))
            .with_stdout(Box::new(LimitedPipe::new(
                stdout_sender,
                stdout_overflow.clone(),
            )))
            .with_stderr(Box::new(LimitedPipe::new(
                stderr_sender,
                stderr_overflow.clone(),
            )));
        let capabilities = runner.capabilities_mut();
        capabilities.insecure_allow_all = false;
        capabilities.threading.max_threads = Some(1);
        capabilities.threading.enable_deep_sleep = false;
        let started = Instant::now();
        let result = runner.run_wasm(
            RuntimeOrEngine::Runtime(Arc::new(runtime)),
            "php",
            module,
            *data.hash(),
        );
        record["php_execution_elapsed_ms"] = json!(started.elapsed().as_millis());
        drop(runner);
        result.context("fixed PHP memory case execution")
    })();
    record["ok"] = json!(outcome.is_ok());
    if let Err(error) = &outcome {
        record["execution_error"] =
            json!(format!("{error:#}").chars().take(4096).collect::<String>());
    }
    drop(guard);
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    record["runtime_shutdown_returned"] = json!(true);
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    record["stdout_eof"] = json!(stdout.is_ok());
    record["stderr_eof"] = json!(stderr.is_ok());
    record["stdout_overflow"] = json!(stdout_overflow.load(Ordering::Relaxed));
    record["stderr_overflow"] = json!(stderr_overflow.load(Ordering::Relaxed));
    if let Ok(bytes) = &stdout {
        record["stdout_bytes"] = json!(bytes.len());
        record["stdout"] = json!(String::from_utf8_lossy(bytes));
    }
    if let Ok(bytes) = &stderr {
        record["stderr_bytes"] = json!(bytes.len());
        record["stderr"] = json!(String::from_utf8_lossy(bytes));
    }
    let memories = memories.lock().expect("memory observations poisoned");
    record["memory_observation_count"] = json!(memories.len());
    if let Some(memory) = memories.first() {
        record["memory"] = json!({"maximum_pages":memory.maximum.map(|p| p.0),
            "size_pages":memory.pages.0,"shared":memory.shared});
    }
    ensure!(
        memories.len() == 1,
        "expected one safe memory recycle observation"
    );
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "case output exceeded finite capture"
    );
    let stdout = stdout.context("case stdout EOF")?;
    let stderr = stderr.context("case stderr EOF")?;
    record["php"] = serde_json::from_slice::<Value>(&stdout)
        .ok()
        .unwrap_or(Value::Null);
    let stderr = String::from_utf8_lossy(&stderr);
    record["oom_observed"] = json!(
        outcome.is_err()
            && stderr.starts_with(READY)
            && stderr.to_ascii_lowercase().contains("out of memory")
    );
    // A run_wasm failure is an observation here, not an automatic diagnostic
    // success. validate() still requires the exact expected OOM conditions.
    Ok(())
}

fn validate(record: &Value, case: Case) -> Result<Option<u64>> {
    let memory = &record["memory"];
    let max = if case.capped { CAP.0 } else { 65536 };
    ensure!(
        memory["maximum_pages"] == max && memory["shared"] == true,
        "actual memory type mismatch"
    );
    let pages = memory["size_pages"]
        .as_u64()
        .context("missing actual memory size")?;
    ensure!(
        (403..=u64::from(max)).contains(&pages),
        "actual linear memory outside bounds"
    );
    ensure!(
        record["ok"] == case.success_expected,
        "unexpected case execution result"
    );
    if !case.success_expected {
        ensure!(
            record["oom_observed"] == true,
            "failure was not the expected allocation OOM with memory_limit=-1"
        );
        ensure!(
            record["php"].is_null(),
            "failed allocation unexpectedly emitted a completed JSON response"
        );
        return Ok(None);
    }
    let php = &record["php"];
    ensure!(
        php["php_version"] == "8.5.12" && php["memory_limit"] == "-1" && php["echo"] == ECHO,
        "fixed PHP response mismatch"
    );
    let width = php["php_int_size"]
        .as_u64()
        .context("invalid PHP integer width")?;
    ensure!(matches!(width, 4 | 8), "invalid PHP integer width");
    if case.allocate {
        ensure!(
            record["stderr"] == READY,
            "allocation control stderr mismatch"
        );
        ensure!(
            php["kind"] == "allocation"
                && php["allocated_bytes"] == ALLOCATION_BYTES
                && php["first_byte"] == 120
                && php["last_byte"] == 120,
            "allocation control did not complete the fixed allocation"
        );
        ensure!(
            pages > u64::from(CAP.0),
            "uncapped control did not exceed capped linear-memory size"
        );
    } else {
        ensure!(
            record["stderr"] == "",
            "normal case wrote unexpected stderr"
        );
        ensure!(
            php["kind"] == "normal"
                && php["allocated_bytes"] == 0
                && php["first_byte"].is_null()
                && php["last_byte"].is_null(),
            "normal response mismatch"
        );
    }
    Ok(Some(width))
}

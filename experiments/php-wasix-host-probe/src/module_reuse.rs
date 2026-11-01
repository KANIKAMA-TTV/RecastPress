//! Compile one fixed PHP module, then instantiate it in three fresh requests.
//! This is a sequential reuse observation, not a product worker or cache.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use wasmer::{Engine, Module};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::{LimitedPipe, OUTPUT_LIMIT, collect_pipe, make_engine};

const ENV_KEYS: [&str; 3] = [
    "RP_MODULE_REUSE_REQUEST_1",
    "RP_MODULE_REUSE_REQUEST_2",
    "RP_MODULE_REUSE_REQUEST_3",
];
const MARKERS: [&str; 3] = [
    "recastpress-v0.5-module-reuse-request-1",
    "recastpress-v0.5-module-reuse-request-2",
    "recastpress-v0.5-module-reuse-request-3",
];
const ECHO: &str = "recastpress-v0.5-module-reuse";
const PHP_PROGRAM: &str = r#"
$rp_before_present = array_key_exists('RP_MODULE_REUSE_STATE', $GLOBALS);
$rp_before_value = $GLOBALS['RP_MODULE_REUSE_STATE'] ?? null;
$rp_markers = [
    'RP_MODULE_REUSE_REQUEST_1' => getenv('RP_MODULE_REUSE_REQUEST_1'),
    'RP_MODULE_REUSE_REQUEST_2' => getenv('RP_MODULE_REUSE_REQUEST_2'),
    'RP_MODULE_REUSE_REQUEST_3' => getenv('RP_MODULE_REUSE_REQUEST_3'),
];
$rp_present = array_values(array_filter($rp_markers, static fn($value) => $value !== false));
$rp_marker = count($rp_present) === 1 ? $rp_present[0] : null;
$GLOBALS['RP_MODULE_REUSE_STATE'] = $rp_marker;
echo json_encode([
    'php_version' => PHP_VERSION,
    'php_int_size' => PHP_INT_SIZE,
    'request_markers' => $rp_markers,
    'request_marker_count' => count($rp_present),
    'globals_before_present' => $rp_before_present,
    'globals_before_value' => $rp_before_value,
    'globals_after' => $GLOBALS['RP_MODULE_REUSE_STATE'],
    'echo' => 'recastpress-v0.5-module-reuse',
], JSON_THROW_ON_ERROR);
"#;

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-module-reuse.v0.5");
    report["scope"] = json!("one-compiled-PHP-module-three-sequential-fresh-environments");
    report["module_reuse_verified"] = json!(false);
    report["request_isolation_verified"] = json!(false);
    report["native_comparison_performed"] = json!(false);
    report["isolation"]["guest_fixture_files"] = json!([]);
    report["isolation"]["wordpress_directory_supplied"] = json!(false);
    report["isolation"]["scratch_directory_supplied"] = json!(false);
    report["reuse"] = json!({
        "requested_requests": 3,
        "php_module_compile_invocations": 0,
        "module_clone_count": 0,
        "completed_requests": 0,
        "request_environment_policy": "fresh Tokio runtime, PluggableRuntime and WasiRunner per request",
        "module_delivery": "Module.clone",
        "request_path_module_compile_calls": 0,
        "counter_scope": "explicit PHP Module::new call site; internal backend operations are not instrumented",
        "source_statement": "Module::new occurs once before the loop; the request path receives a Module clone and never loads Wasm bytes"
    });
    report["requests"] = json!([]);
}

pub fn verify(bytes: Vec<u8>, report: &mut Value) -> Result<()> {
    let data = HashedModuleData::new(bytes);
    let engine = make_engine();
    let engine_identity = format!("{:?}", engine.id());
    report["reuse"]["configured_engine_identity"] = json!(engine_identity);
    report["phase"] = json!("module_compile");
    eprintln!("PHP_WASIX_PHASE=module_compile");
    let started = Instant::now();
    report["reuse"]["php_module_compile_invocations"] = json!(1);
    // This is the only PHP compiler entry point in the reuse path. Keep it
    // outside all request runtimes, so no shared runtime retains their pipes.
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile verified PHP module once for reuse")?;

    let mut integer_width = None;
    for index in 0..MARKERS.len() {
        let number = index + 1;
        report["phase"] = json!(format!("module_reuse_request_{number}"));
        eprintln!("PHP_WASIX_PHASE=module_reuse_request_{number}");
        report["reuse"]["module_clone_count"] = json!(number);
        let mut request = json!({
            "request_number": number,
            "configured_engine_identity": engine_identity,
            "module_delivery": "Module.clone",
            "module_compile_invocations": 0,
            "guest_environment_supplied": [{"key": ENV_KEYS[index], "value": MARKERS[index]}],
            "host_environment_forwarded": false,
            "host_directories_mapped": false,
            "home_mapped": false,
            "networking": "UnsupportedVirtualNetworking",
            "http_client": false,
            "stdin": "closed-pipe",
            "stdout_limit_bytes": OUTPUT_LIMIT,
            "stderr_limit_bytes": OUTPUT_LIMIT,
            "php_completed": false,
            "isolation_verified": false
        });
        let outcome = execute_request(&engine, module.clone(), &data, index, &mut request)
            .and_then(|php| {
                request["php"] = php.clone();
                let width = validate(&php, index)?;
                if let Some(previous) = integer_width {
                    ensure!(
                        width == previous,
                        "PHP integer width changed between requests"
                    );
                }
                integer_width = Some(width);
                request["isolation_verified"] = json!(true);
                Ok(())
            });
        if let Err(error) = &outcome {
            request["error"] = json!(format!("{error:#}").chars().take(4096).collect::<String>());
        }
        report["requests"]
            .as_array_mut()
            .expect("describe initialized requests")
            .push(request);
        outcome?;
        report["reuse"]["completed_requests"] = json!(number);
    }
    report["module_reuse_verified"] = json!(true);
    report["request_isolation_verified"] = json!(true);
    report["observed_php_int_size"] = json!(integer_width);
    Ok(())
}

fn execute_request(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    index: usize,
    request: &mut Value,
) -> Result<Value> {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create fresh request Tokio runtime")?;
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
        // Reuse the configured engine that compiled the supplied Module. No
        // load_hashed_module/Module::new call exists in this request path.
        runtime.set_engine(engine.clone());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let mut runner = WasiRunner::new();
        runner
            .with_args(["-n", "-r", PHP_PROGRAM])
            .with_envs([(ENV_KEYS[index], MARKERS[index])])
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
        request["php_execution_elapsed_ms"] = json!(started.elapsed().as_millis());
        request["php_completed"] = json!(result.is_ok());
        drop(runner);
        result.context("execute supplied compiled PHP module in fresh request")
    })();

    drop(guard);
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    request["runtime_shutdown_returned"] = json!(true);
    request["runtime_shutdown_wait_limit_ms"] = json!(2000);
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    request["stdout_eof"] = json!(stdout.is_ok());
    request["stderr_eof"] = json!(stderr.is_ok());
    request["stdout_overflow"] = json!(stdout_overflow.load(Ordering::Relaxed));
    request["stderr_overflow"] = json!(stderr_overflow.load(Ordering::Relaxed));
    if let Ok(bytes) = &stdout {
        request["stdout_bytes"] = json!(bytes.len());
        request["stdout"] = json!(String::from_utf8_lossy(bytes));
    }
    if let Ok(bytes) = &stderr {
        request["stderr_bytes"] = json!(bytes.len());
        request["stderr"] = json!(String::from_utf8_lossy(bytes));
    }
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed),
        "request stdout limit exceeded"
    );
    ensure!(
        !stderr_overflow.load(Ordering::Relaxed),
        "request stderr limit exceeded"
    );
    let stdout = stdout.context("collect request stdout through EOF")?;
    let stderr = stderr.context("collect request stderr through EOF")?;
    ensure!(stderr.is_empty(), "request wrote unexpected stderr");
    serde_json::from_slice(&stdout).context("decode fixed request PHP output")
}

fn validate(php: &Value, index: usize) -> Result<u64> {
    ensure!(php["php_version"] == "8.5.12", "unexpected PHP version");
    let width = php["php_int_size"]
        .as_u64()
        .context("PHP integer width is not an integer")?;
    ensure!(matches!(width, 4 | 8), "unexpected PHP integer width");
    ensure!(php["echo"] == ECHO, "fixed echo mismatch");
    ensure!(
        php["globals_before_present"] == false,
        "previous request global is present"
    );
    ensure!(
        php["globals_before_value"].is_null(),
        "previous request global has a value"
    );
    ensure!(
        php["globals_after"] == MARKERS[index],
        "current request global marker mismatch"
    );
    ensure!(
        php["request_marker_count"] == 1,
        "guest observed more or fewer than one fixed request marker"
    );
    for (key_index, key) in ENV_KEYS.iter().enumerate() {
        let expected = if key_index == index {
            json!(MARKERS[index])
        } else {
            json!(false)
        };
        ensure!(
            php["request_markers"][key] == expected,
            "guest request marker was missing or leaked between requests: {key}"
        );
    }
    Ok(width)
}

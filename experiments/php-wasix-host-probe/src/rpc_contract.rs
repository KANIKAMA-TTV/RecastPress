//! Fixed real-PHP-to-Rust RPC diagnostic. Not wpdb, a real DB or a product ABI.

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

use super::rpc_fs::{
    MAX_FRAME_BYTES, MAX_OPERATIONS, MAX_REPLY_READ_BYTES, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
    RpcFs, Snapshot,
};
use super::{LimitedPipe, OUTPUT_LIMIT, collect_pipe, make_engine};

const MOUNT: &str = "/rp-p0-rpc";
const CHANNEL: &str = "/rp-p0-rpc/channel";
const ENV_KEY: &str = "RP_RPC_REQUEST_MARKER";
const LITERAL: &[u8] = b"RP_P0 CACHE_SET 7\n\0\xff";
const INCOMPLETE_WIRE: &[u8] = b"RP_P0 CACHE_";
const CASES: [&str; 4] = [
    "normal_first",
    "normal_second",
    "incomplete_ignored",
    "normal_recovery",
];
const MARKERS: [&str; 4] = [
    "recastpress-v0.5-rpc-request-1",
    "recastpress-v0.5-rpc-request-2",
    "recastpress-v0.5-rpc-request-3",
    "recastpress-v0.5-rpc-request-4",
];
const COMMANDS: [&[u8]; 5] = [
    b"RP_P0 CACHE_GET\n",
    b"RP_P0 CACHE_SET 7\n",
    b"RP_P0 CACHE_GET\n",
    b"RP_P0 DB_SELECT_FIXTURE\n",
    b"RP_P0 CACHE_GET\n",
];
const NORMAL: &str = r#"
function rp_rpc($channel, $command) {
    if (!rewind($channel)) { throw new RuntimeException('RPC rewind before write failed'); }
    $wire = 'RP_P0 ' . $command . "\n";
    $first = substr($wire, 0, 9);
    $second = substr($wire, 9);
    if (fwrite($channel, $first) !== strlen($first) || !fflush($channel) ||
        fwrite($channel, $second) !== strlen($second) || !fflush($channel)) {
        throw new RuntimeException('RPC fragmented write failed');
    }
    if (!rewind($channel)) { throw new RuntimeException('RPC rewind before read failed'); }
    $reply = fgets($channel, 1025);
    if ($reply === false || !str_ends_with($reply, "\n")) { throw new RuntimeException('RPC reply missing'); }
    return json_decode($reply, true, 32, JSON_THROW_ON_ERROR);
}
$before = array_key_exists('RP_RPC_STATE', $GLOBALS);
$marker = getenv('RP_RPC_REQUEST_MARKER');
$GLOBALS['RP_RPC_STATE'] = $marker;
$channel = fopen('/rp-p0-rpc/channel', 'r+b');
if ($channel === false) { throw new RuntimeException('RPC open failed'); }
$miss = rp_rpc($channel, 'CACHE_GET');
$set = rp_rpc($channel, 'CACHE_SET 7');
$hit = rp_rpc($channel, 'CACHE_GET');
$db = rp_rpc($channel, 'DB_SELECT_FIXTURE');
if (!fclose($channel)) { throw new RuntimeException('RPC close failed'); }
$channel = fopen('/rp-p0-rpc/channel', 'r+b');
if ($channel === false) { throw new RuntimeException('RPC reopen failed'); }
$reopened = rp_rpc($channel, 'CACHE_GET');
if (!fclose($channel)) { throw new RuntimeException('RPC second close failed'); }
echo "RP_P0 CACHE_SET 7\n\x00\xff";
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
    'kind'=>'normal','marker'=>$marker,'globals_before_present'=>$before,
    'globals_after'=>$GLOBALS['RP_RPC_STATE'],'cache_before'=>$miss,'cache_set'=>$set,
    'cache_after'=>$hit,'database'=>$db,'reopened_cache'=>$reopened,'channel_closed'=>true,
    'echo'=>'recastpress-v0.5-private-rpc'], JSON_THROW_ON_ERROR);
"#;
const INCOMPLETE: &str = r#"
$channel = fopen('/rp-p0-rpc/channel', 'r+b');
if ($channel === false) { throw new RuntimeException('RPC open failed'); }
$wire = 'RP_P0 CACHE_';
$written = fwrite($channel, $wire);
$flushed = fflush($channel);
$closed = fclose($channel);
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
    'kind'=>'incomplete_ignored','marker'=>getenv('RP_RPC_REQUEST_MARKER'),
    'wire_hex'=>bin2hex($wire),'written'=>$written,'flushed'=>$flushed,'channel_closed'=>$closed,
    'echo'=>'recastpress-v0.5-private-rpc'], JSON_THROW_ON_ERROR);
"#;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-rpc-contract.v0.5");
    report["scope"] = json!("fixed-PHP-private-RPC-four-request-diagnostic-not-product-ABI");
    report["rpc_contract_verified"] = json!(false);
    report["request_isolation_verified"] = json!(false);
    report["stdout_not_rpc_verified"] = json!(false);
    report["incomplete_frame_rejected"] = json!(false);
    report["standard_wpdb"] = json!(false);
    report["native_comparison_performed"] = json!(false);
    report["product_adopted"] = json!(false);
    report["isolation"]["guest_fixture_files"] = json!([]);
    report["isolation"]["guest_private_mounts"] = json!([MOUNT]);
    report["isolation"]["rpc_channel"] = json!(CHANNEL);
    report["isolation"]["guest_environment_policy"] = json!("one fixed per-request marker only");
    report["isolation"]["other_virtual_root_writes_forbidden"] = json!(false);
    report["rpc_limits"] = json!({"frame_bytes":MAX_FRAME_BYTES,"single_write_bytes":MAX_FRAME_BYTES,
        "request_bytes":MAX_REQUEST_BYTES,"operations":MAX_OPERATIONS,"response_bytes":MAX_RESPONSE_BYTES,
        "total_generated_response_bytes":MAX_OPERATIONS * MAX_RESPONSE_BYTES,
        "reply_read_bytes":MAX_REPLY_READ_BYTES,"simultaneous_channels":1});
    report["reuse"] = json!({"php_module_compile_invocations":0,"module_clone_count":0,"observed_cases":0,
        "request_path_module_compile_calls":0,"module_delivery":"Module.clone",
        "counter_scope":"explicit PHP Module::new call site, not internal backend instrumentation"});
    report["cases"] = json!([]);
    report["verdict_scope"] = json!(
        "Top-level ok describes the diagnostic; the ignored incomplete-frame case stays ok=false, even though PHP exits zero."
    );
    report["limitations"] = json!([
        "Only CACHE_GET, CACHE_SET 7 and DB_SELECT_FIXTURE are accepted on a request-local private virtual file.",
        "The DB returns fixed nonce rows; no real DB, SQL parser, standard wpdb, WordPress or product ABI is implemented.",
        "No memory, RSS, aggregate, CPU or in-process deadline limit is established; the owned outer process must exit and be reaped.",
        "Only four sequential fixed PHP programs are checked; PHP compatibility and concurrent product workers remain unverified.",
        "The immutable host filesystem and network are not supplied; WasiRunner's other temporary virtual-root writes are not forbidden here."
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
    let module = compiled.context("compile verified PHP artifact once for private RPC")?;
    let mut all_met = true;
    let mut width = None;
    for (index, name) in CASES.iter().enumerate() {
        report["phase"] = json!(name);
        eprintln!("PHP_WASIX_PHASE={name}");
        report["reuse"]["module_clone_count"] = json!(index + 1);
        let mut record = json!({"name":name,"request_number":index + 1,"nonce":101 + index,
            "configured_engine_identity":identity,"module_delivery":"Module.clone","module_compile_invocations":0,
            "success_expected":index != 2,"ok":false,"php_completed":false,"rpc_valid":false,"expectation_met":false,
            "ok_scope":"guest execution plus RPC finish and capture validation; not the diagnostic verdict",
            "guest_environment_supplied":[[ENV_KEY,MARKERS[index]]],"host_environment_forwarded":false,
            "host_directories_mapped":false,"home_mapped":false,"http_client":false,
            "networking":"UnsupportedVirtualNetworking","private_rpc_mount":MOUNT,"rpc_channel":CHANNEL,
            "stdin":"closed-pipe","stdout_limit_bytes":OUTPUT_LIMIT,"stderr_limit_bytes":OUTPUT_LIMIT});
        let result = execute(&engine, module.clone(), &data, index, &mut record)
            .and_then(|()| validate(&record, index))
            .and_then(|observed| {
                if let Some(previous) = width {
                    ensure!(previous == observed, "PHP integer width changed");
                }
                width = Some(observed);
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
        "the fixed RPC diagnostic did not meet every expectation"
    );
    report["observed_php_int_size"] = json!(width);
    for field in [
        "rpc_contract_verified",
        "request_isolation_verified",
        "stdout_not_rpc_verified",
        "incomplete_frame_rejected",
    ] {
        report[field] = json!(true);
    }
    Ok(())
}

fn snapshot_value(snapshot: &Snapshot) -> Value {
    json!({"operations":snapshot.operations,"request_bytes":snapshot.request_bytes,
        "response_bytes":snapshot.response_bytes,"response_read_bytes":snapshot.response_read_bytes,
        "pending_bytes":snapshot.pending_bytes,
        "cache_entries":snapshot.cache_entries,"channel_open":snapshot.channel_open,
        "finished":snapshot.finished,"failure":snapshot.failure,
        "real_db":snapshot.real_db,"standard_wpdb":snapshot.standard_wpdb})
}

fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    index: usize,
    record: &mut Value,
) -> Result<()> {
    let fs = RpcFs::new(101 + index as u64);
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
            .with_args(["-n", "-r", if index == 2 { INCOMPLETE } else { NORMAL }])
            .with_envs([(ENV_KEY, MARKERS[index])])
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(false)
            .with_mount(MOUNT.into(), Arc::new(fs.clone()))
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
        result.context("fixed PHP private RPC execution")
    })();
    record["php_completed"] = json!(outcome.is_ok());
    if let Err(error) = &outcome {
        record["execution_error"] =
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
    if let Ok(wire) = &stdout {
        record["stdout_bytes"] = json!(wire.len());
        record["stdout_hex"] = json!(hex(wire));
    }
    if let Ok(wire) = &stderr {
        record["stderr_bytes"] = json!(wire.len());
        record["stderr_hex"] = json!(hex(wire));
    }
    let finished = fs.finish();
    record["rpc_valid"] = json!(finished.is_ok());
    if let Err(error) = &finished {
        record["rpc_error"] = json!(format!("{error:#}").chars().take(4096).collect::<String>());
    }
    record["rpc"] = snapshot_value(&fs.snapshot()?);
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "RPC case capture exceeded limit"
    );
    let stdout = stdout.context("RPC stdout EOF")?;
    ensure!(
        stderr.context("RPC stderr EOF")?.is_empty(),
        "RPC case wrote unexpected stderr"
    );
    let body = if index == 2 {
        stdout.as_slice()
    } else {
        stdout
            .strip_prefix(LITERAL)
            .context("literal RPC-like stdout prefix missing")?
    };
    record["stdout_literal_hex"] = json!(if index == 2 {
        String::new()
    } else {
        hex(LITERAL)
    });
    record["php"] = serde_json::from_slice::<Value>(body).context("RPC PHP JSON output")?;
    record["ok"] = json!(finished.is_ok());
    Ok(())
}

fn expected_php(index: usize, width: u64) -> Value {
    if index == 2 {
        json!({"php_version":"8.5.12","php_int_size":width,"kind":"incomplete_ignored","marker":MARKERS[index],
            "wire_hex":hex(INCOMPLETE_WIRE),"written":INCOMPLETE_WIRE.len(),"flushed":true,
            "channel_closed":true,"echo":"recastpress-v0.5-private-rpc"})
    } else {
        json!({"php_version":"8.5.12","php_int_size":width,"kind":"normal","marker":MARKERS[index],
            "globals_before_present":false,"globals_after":MARKERS[index],"cache_before":{"value":null},
            "cache_set":{"ok":true},"cache_after":{"value":7},"database":{"rows":[{"nonce":101+index}]},
            "reopened_cache":{"value":7},"channel_closed":true,"echo":"recastpress-v0.5-private-rpc"})
    }
}

fn validate(record: &Value, index: usize) -> Result<u64> {
    ensure!(
        record["php_completed"] == true,
        "PHP itself must exit successfully, including the ignored incomplete case"
    );
    ensure!(
        record["ok"] == (index != 2) && record["rpc_valid"] == (index != 2),
        "RPC acceptance mismatch"
    );
    let rpc = &record["rpc"];
    ensure!(
        rpc["finished"] == true && rpc["channel_open"] == false,
        "RPC channel not closed/sealed"
    );
    ensure!(
        rpc["real_db"] == false && rpc["standard_wpdb"] == false,
        "fixture DB scope changed"
    );
    if index == 2 {
        ensure!(
            rpc["operations"] == 0
                && rpc["request_bytes"] == INCOMPLETE_WIRE.len()
                && rpc["pending_bytes"] == INCOMPLETE_WIRE.len()
                && rpc["cache_entries"] == 0
                && rpc["response_bytes"] == 0
                && rpc["response_read_bytes"] == 0,
            "incomplete RPC unexpectedly dispatched"
        );
        ensure!(
            rpc["failure"] == "incomplete RPC frame on channel close",
            "failure was not incomplete-frame close rejection"
        );
        ensure!(
            record["rpc_error"]
                .as_str()
                .is_some_and(|error| error.contains("incomplete RPC frame")),
            "missing retained RPC rejection"
        );
    } else {
        ensure!(
            rpc["failure"].is_null() && rpc["pending_bytes"] == 0 && rpc["cache_entries"] == 1,
            "normal RPC state invalid"
        );
        ensure!(
            rpc["operations"] == COMMANDS.len()
                && rpc["request_bytes"] == COMMANDS.iter().map(|wire| wire.len()).sum::<usize>(),
            "Rust callback count/input bytes changed; stdout must not become RPC"
        );
        let bytes = rpc["response_bytes"]
            .as_u64()
            .context("missing generated reply bytes")?;
        ensure!(
            bytes > 0 && bytes <= (MAX_OPERATIONS * MAX_RESPONSE_BYTES) as u64,
            "reply budget exceeded"
        );
        ensure!(
            rpc["response_read_bytes"] == bytes && bytes <= MAX_REPLY_READ_BYTES as u64,
            "normal PHP must read each generated reply exactly once"
        );
    }
    let width = record["php"]["php_int_size"]
        .as_u64()
        .context("invalid PHP integer width")?;
    ensure!(matches!(width, 4 | 8), "invalid PHP integer width");
    ensure!(
        record["php"] == expected_php(index, width),
        "PHP RPC observation differs from fixed contract"
    );
    Ok(width)
}

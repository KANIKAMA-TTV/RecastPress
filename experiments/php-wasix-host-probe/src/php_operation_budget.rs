//! Closed PHP Wasm-operation-budget trial, not a CPU-time or product limit.
//! Only this mode creates a metered compiler. Other modes remain uninstrumented.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::sys::CompilerConfig;
use wasmer::wasmparser::Operator;
use wasmer::{
    AsStoreMut, Engine, GlobalType, Instance, Memory, Module, Mutability, StoreMut, Type,
};
use wasmer_middlewares::{
    Metering,
    metering::{MeteringPoints, get_remaining_points},
};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::runtime::task_manager::{SpawnType, TaskWasm, TaskWasmRecycleProperties};
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking, VirtualTaskManager, WasiThreadError,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::{LimitedPipe, OUTPUT_LIMIT, collect_pipe};

const POINTS: u64 = 100_000_000;
const READY: &[u8] = b"RP_PHP_OPERATION_BUDGET_READY\n";
const NAMES: [&str; 3] = ["normal_first", "loop_exhausted", "normal_recovery"];
const NORMAL: &str = r#"
$before = array_key_exists('RP_OPERATION_STATE', $GLOBALS);
$GLOBALS['RP_OPERATION_STATE'] = 'request-local';
echo json_encode(['php_version'=>PHP_VERSION,'version_id'=>PHP_VERSION_ID,
'php_int_size'=>PHP_INT_SIZE,'sapi'=>PHP_SAPI,'kind'=>'normal',
'globals_before_present'=>$before,'globals_after'=>$GLOBALS['RP_OPERATION_STATE'],
'echo'=>'recastpress-v0.5-php-operation-budget'], JSON_THROW_ON_ERROR);
"#;
const LOOP: &str = r#"
fwrite(STDOUT, "RP_PHP_OPERATION_BUDGET_READY\n");
fflush(STDOUT);
while (true) {}
"#;

fn sha(wire: &[u8]) -> String {
    format!("{:x}", Sha256::digest(wire))
}
fn hex(wire: &[u8]) -> String {
    wire.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-operation-budget.v0.5");
    report["scope"] = json!("fixed-three-case-PHP-Wasm-operation-budget-trial-not-product-limit");
    report["php_wasm_operation_budget_verified_for_fixed_cases"] = json!(false);
    for name in [
        "production_operation_budget_verified",
        "cpu_time_budget_verified",
        "php_opcode_budget_verified",
        "hostcall_timeout_verified",
        "production_memory_limit_verified",
        "fiber_compatibility_verified",
        "native_comparison_performed",
        "product_adopted",
    ] {
        report[name] = json!(false);
    }
    report["trial_points"] = json!(POINTS);
    report["cost_policy"] = json!("one-point-per-original-Wasm-operator");
    report["isolation"]["guest_environment_supplied"] = json!([]);
    report["isolation"]["guest_fixture_files"] = json!([]);
    report["isolation"]["other_virtual_root_writes_forbidden"] = json!(false);
    report["reuse"] = json!({"php_module_compile_invocations":0,"module_clone_count":0,
        "request_path_module_compile_calls":0,"observed_cases":0,"module_delivery":"Module.clone",
        "counter_scope":"explicit PHP Module::new call site, not internal backend instrumentation"});
    report["cases"] = json!([]);
    report["verdict_scope"] = json!(
        "Top-level ok describes all three diagnostic expectations; the loop remains ok=false."
    );
    report["limitations"] = json!([
        "100000000 is a fixed experiment value, not a product budget or wall-clock deadline.",
        "Metering weights original Wasm operators, not PHP opcodes, native instructions or CPU seconds.",
        "Hostcall duration, blocking IO, compiler work, RSS and guest linear memory are not bounded by these points.",
        "Only this closed mode instruments its single PHP module; other host modes are unchanged.",
        "The outer owned-child supervisor must enforce its deadline and confirm host exit/reap.",
        "WordPress, real DB, Fiber and full PHP compatibility are not exercised or established."
    ]);
}

fn engine() -> Engine {
    // Metering stores module-specific indexes. This compiler is used to compile
    // exactly one PHP Module; every request receives only a Module clone.
    let middleware = Arc::new(Metering::new(POINTS, |_operator: &Operator| 1));
    #[cfg(feature = "llvm")]
    let mut compiler = wasmer_compiler_llvm::LLVM::default();
    #[cfg(all(not(feature = "llvm"), feature = "cranelift"))]
    let mut compiler = wasmer_compiler_cranelift::Cranelift::default();
    compiler.push_middleware(middleware);
    compiler.into()
}

fn read_points(store: &mut impl AsStoreMut, instance: &Instance) -> Result<Value> {
    // The upstream getter panics on absent/wrong globals. Check their safe
    // exported types before invoking it; an observation failure cannot pass.
    for (name, ty) in [
        ("wasmer_metering_remaining_points", Type::I64),
        ("wasmer_metering_points_exhausted", Type::I32),
    ] {
        let global = instance
            .exports
            .get_global(name)
            .context("missing metering global")?;
        ensure!(
            global.ty(store) == GlobalType::new(ty, Mutability::Var),
            "wrong metering global type"
        );
    }
    Ok(match get_remaining_points(store, instance) {
        MeteringPoints::Remaining(points) => json!({"kind":"remaining","points":points}),
        MeteringPoints::Exhausted => json!({"kind":"exhausted"}),
    })
}

#[derive(Debug, Default)]
struct Observations {
    setup_calls: usize,
    recycle_calls: usize,
    initial: Option<Value>,
    final_points: Option<Value>,
    error: Option<String>,
}
type SharedObservations = Arc<Mutex<Observations>>;

fn observe_setup(
    state: &SharedObservations,
    store: &mut StoreMut,
    instance: &Instance,
) -> Result<()> {
    {
        let mut state = state
            .lock()
            .map_err(|_| anyhow::anyhow!("metering setup observation mutex poisoned"))?;
        state.setup_calls = state.setup_calls.saturating_add(1);
        if state.setup_calls != 1 {
            state
                .error
                .get_or_insert("more than one instance setup callback".into());
            anyhow::bail!("more than one instance setup callback");
        }
    }
    let observed = read_points(store, instance).and_then(|points| {
        ensure!(
            points == json!({"kind":"remaining","points":POINTS}),
            "fresh instance budget is not the fixed initial value"
        );
        Ok(points)
    });
    let mut state = state
        .lock()
        .map_err(|_| anyhow::anyhow!("metering setup observation mutex poisoned"))?;
    match observed {
        Ok(points) => {
            state.initial = Some(points);
            Ok(())
        }
        Err(error) => {
            state.error.get_or_insert(format!("{error:#}"));
            Err(error)
        }
    }
}

fn observe_recycle(state: &SharedObservations, props: &mut TaskWasmRecycleProperties) {
    let observed = (|| {
        let instance = props
            .env
            .try_clone_instance()
            .context("recycle did not expose one static instance")?;
        read_points(&mut props.store, &instance)
    })();
    // Poisoning itself makes the later locked snapshot fail closed. Never
    // invent Remaining/Exhausted when the observation cannot be obtained.
    if let Ok(mut state) = state.lock() {
        state.recycle_calls = state.recycle_calls.saturating_add(1);
        if state.recycle_calls != 1 {
            state
                .error
                .get_or_insert("more than one recycle callback".into());
        }
        match observed {
            Ok(points) => {
                if state.final_points.is_none() {
                    state.final_points = Some(points);
                }
            }
            Err(error) => {
                state.error.get_or_insert(format!("{error:#}"));
            }
        }
    }
}

type SharedTask =
    Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send + 'static>;

struct MeteredTaskManager {
    inner: TokioTaskManager,
    observations: SharedObservations,
}
impl fmt::Debug for MeteredTaskManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeteredTaskManager").finish_non_exhaustive()
    }
}
impl VirtualTaskManager for MeteredTaskManager {
    fn build_memory(
        &self,
        store: &mut StoreMut,
        spawn: SpawnType,
    ) -> Result<Option<Memory>, WasiThreadError> {
        self.inner.build_memory(store, spawn)
    }
    fn sleep_now(
        &self,
        time: Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>> {
        self.inner.sleep_now(time)
    }
    fn task_shared(&self, task: SharedTask) -> Result<(), WasiThreadError> {
        self.inner.task_shared(task)
    }
    fn task_wasm(&self, mut task: TaskWasm) -> Result<(), WasiThreadError> {
        // Resuming store snapshots could restore a spent global. This closed
        // no-snapshot/no-deep-sleep mode accepts only new static executions.
        if task.globals.is_some()
            || task.callbacks.trigger.is_some()
            || task.module.info().start_function.is_some()
        {
            return Err(WasiThreadError::Unsupported);
        }
        let previous = task.callbacks.recycle.take();
        let observations = self.observations.clone();
        task.callbacks.recycle = Some(Box::new(move |mut props| {
            observe_recycle(&observations, &mut props);
            if let Some(previous) = previous {
                previous(props);
            }
        }));
        self.inner.task_wasm(task)
    }
    fn task_dedicated(
        &self,
        task: Box<dyn FnOnce() + Send + 'static>,
    ) -> Result<(), WasiThreadError> {
        self.inner.task_dedicated(task)
    }
    fn thread_parallelism(&self) -> Result<usize, WasiThreadError> {
        self.inner.thread_parallelism()
    }
}

pub fn verify(bytes: Vec<u8>, report: &mut Value) -> Result<()> {
    ensure!(
        cfg!(target_os = "linux"),
        "fixed PHP operation-budget execution is Linux-only; MSVC formal EH remains unexecuted"
    );
    let data = HashedModuleData::new(bytes);
    let engine = engine();
    let identity = format!("{:?}", engine.id());
    report["reuse"]["configured_engine_identity"] = json!(identity);
    report["phase"] = json!("module_compile");
    eprintln!("PHP_WASIX_PHASE=module_compile");
    report["reuse"]["php_module_compile_invocations"] = json!(1);
    let started = Instant::now();
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile the fixed PHP module once with operation metering")?;
    report["module_start_section_present"] = json!(module.info().start_function.is_some());
    ensure!(
        module.info().start_function.is_none(),
        "operation trial rejects Wasm start sections before setup observation"
    );
    let mut all_met = true;
    for (index, name) in NAMES.iter().enumerate() {
        report["phase"] = json!(name);
        eprintln!("PHP_WASIX_PHASE={name}");
        report["reuse"]["module_clone_count"] = json!(index + 1);
        let program = if index == 1 { LOOP } else { NORMAL };
        let mut record = json!({"name":name,"request_number":index + 1,"configured_engine_identity":identity,
            "module_delivery":"Module.clone","module_compile_invocations":0,
            "fixed_program_sha256":sha(program.as_bytes()),"guest_arguments":["-n","-r",if index == 1 {"<fixed-loop>"} else {"<fixed-normal>"}],
            "success_expected":index != 1,"ok":false,"expectation_met":false,
            "ok_scope":"WasiRunner outcome only; the expected exhausted loop remains false",
            "initial_points":null,"final_points":null,"setup_observation_count":0,"recycle_observation_count":0,
            "metering_observation_error":null,"ready_reached":false,"budget_exhausted":false,"php":null,
            "guest_environment_supplied":[],"host_environment_forwarded":false,"host_directories_mapped":false,
            "home_mapped":false,"networking":"UnsupportedVirtualNetworking","http_client":false,
            "stdin":"closed-pipe","stdout_limit_bytes":OUTPUT_LIMIT,"stderr_limit_bytes":OUTPUT_LIMIT});
        let result = execute(&engine, module.clone(), &data, index, &mut record)
            .and_then(|()| validate(&record, index));
        record["expectation_met"] = json!(result.is_ok());
        if let Err(error) = result {
            all_met = false;
            record["expectation_error"] =
                json!(format!("{error:#}").chars().take(4096).collect::<String>());
        }
        report["cases"]
            .as_array_mut()
            .context("cases not initialized")?
            .push(record);
        report["reuse"]["observed_cases"] = json!(index + 1);
    }
    ensure!(
        all_met,
        "fixed PHP operation-budget expectations were not all met"
    );
    report["php_wasm_operation_budget_verified_for_fixed_cases"] = json!(true);
    Ok(())
}

fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    index: usize,
    record: &mut Value,
) -> Result<()> {
    let observations: SharedObservations = Arc::new(Mutex::new(Observations::default()));
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
        let tasks = MeteredTaskManager {
            inner: TokioTaskManager::new(tokio_runtime.handle().clone()),
            observations: observations.clone(),
        };
        let mut runtime = PluggableRuntime::new(Arc::new(tasks));
        runtime.set_engine(engine.clone());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let setup = observations.clone();
        runtime.with_instance_setup(move |_, store, instance, _| {
            observe_setup(&setup, store, instance)
        });
        let mut runner = WasiRunner::new();
        runner
            .with_args(["-n", "-r", if index == 1 { LOOP } else { NORMAL }])
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
        result.context("fixed PHP operation-budget execution")
    })();
    record["ok"] = json!(outcome.is_ok());
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
    for (name, wire) in [("stdout", &stdout), ("stderr", &stderr)] {
        if let Ok(wire) = wire {
            record[format!("{name}_bytes")] = json!(wire.len());
            record[format!("{name}_hex")] = json!(hex(wire));
            record[format!("{name}_sha256")] = json!(sha(wire));
        }
    }
    {
        let observation = observations
            .lock()
            .map_err(|_| anyhow::anyhow!("metering observation snapshot poisoned"))?;
        record["setup_observation_count"] = json!(observation.setup_calls);
        record["recycle_observation_count"] = json!(observation.recycle_calls);
        record["initial_points"] = json!(observation.initial);
        record["final_points"] = json!(observation.final_points);
        record["metering_observation_error"] = json!(observation.error);
        record["budget_exhausted"] =
            json!(observation.final_points == Some(json!({"kind":"exhausted"})));
    }
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "operation-budget capture limit"
    );
    let stdout = stdout.context("operation-budget stdout EOF")?;
    ensure!(
        stderr.context("operation-budget stderr EOF")?.is_empty(),
        "PHP wrote unexpected guest stderr"
    );
    record["ready_reached"] = json!(stdout == READY);
    if index != 1 && outcome.is_ok() {
        record["php"] =
            serde_json::from_slice::<Value>(&stdout).context("normal PHP operation-budget JSON")?;
    }
    // Expected guest errors are preserved above and classified only by validate,
    // which requires READY plus the independently observed exhausted global.
    Ok(())
}

fn validate(record: &Value, index: usize) -> Result<()> {
    ensure!(
        record["setup_observation_count"] == 1
            && record["recycle_observation_count"] == 1
            && record["metering_observation_error"].is_null(),
        "missing/repeated/failed safe metering observations"
    );
    ensure!(
        record["initial_points"] == json!({"kind":"remaining","points":POINTS}),
        "instance did not start with fixed budget"
    );
    ensure!(
        record["ok"] == (index != 1),
        "unexpected PHP execution outcome"
    );
    if index == 1 {
        ensure!(
            record["ready_reached"] == true
                && record["budget_exhausted"] == true
                && record["final_points"] == json!({"kind":"exhausted"}),
            "loop did not reach READY and actual metering exhaustion"
        );
        ensure!(
            record["php"].is_null()
                && record["execution_error"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()),
            "expected loop failure lacks its actual Runner error"
        );
    } else {
        ensure!(
            record["budget_exhausted"] == false && record["ready_reached"] == false,
            "normal case exhausted or printed READY"
        );
        let points = record["final_points"]["points"]
            .as_u64()
            .context("normal case missing remaining points")?;
        ensure!(
            record["final_points"]["kind"] == "remaining" && points > 0 && points < POINTS,
            "normal case did not consume a positive bounded number of points"
        );
        ensure!(
            record["php"]
                == json!({"php_version":"8.5.12","version_id":80512,"php_int_size":8,"sapi":"cli",
            "kind":"normal","globals_before_present":false,"globals_after":"request-local",
            "echo":"recastpress-v0.5-php-operation-budget"}),
            "normal PHP response differs from the fixed contract"
        );
    }
    Ok(())
}

//! Closed PHP Wasm-operation-budget trial, not a CPU-time or product limit.
//! Only this mode creates a metered compiler. Other modes remain uninstrumented.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::sys::CompilerConfig;
use wasmer::wasmparser::Operator;
use wasmer::{
    AsStoreMut, Engine, ExportIndex, Global, GlobalInit, GlobalType, Instance, Memory, Module,
    Mutability, StoreMut, Type,
};
use wasmer_middlewares::Metering;
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::runtime::task_manager::{SpawnType, TaskWasm, TaskWasmRecycleProperties};
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking, VirtualTaskManager, WasiThreadError,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::{LimitedPipe, OUTPUT_LIMIT, collect_pipe};

const POINTS: u64 = 100_000_000;
const OBSERVER_STRATEGY: &str = "setup-captured-typed-global-handles";
const OBSERVER_IDENTITY: &str = "sha256-of-pinned-wasmer-7.2.0-Global-Debug";
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

const CGI_FIXTURE: &[u8] =
    include_bytes!("../../php-wasix-managed-access-probe/fixtures/operation_cgi_contract.php");
const CGI_FIXTURE_SHA: &str = "455204fd5af80680dca3b32de236bb22db2a212ab91f124fc2fb02b8007a1f88";
const CGI_FILENAME: &str = "operation_cgi_contract.php";
const CGI_GUEST: &str = "/fixture/operation_cgi_contract.php";
const CGI_SOURCE: &str =
    "experiments/php-wasix-managed-access-probe/fixtures/operation_cgi_contract.php";

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryProfile {
    Cli,
    ManagedCgi,
}

fn cgi_environment(index: usize) -> Vec<(String, String)> {
    [
        ("GATEWAY_INTERFACE", "CGI/1.1"),
        ("SERVER_PROTOCOL", "HTTP/1.1"),
        (
            "SERVER_SOFTWARE",
            "recastpress-v0.5-fixed-operation-cgi-probe",
        ),
        ("SERVER_NAME", "fixture.invalid"),
        ("SERVER_PORT", "80"),
        ("REDIRECT_STATUS", "200"),
        ("REQUEST_METHOD", "GET"),
        ("SCRIPT_FILENAME", CGI_GUEST),
        ("SCRIPT_NAME", "/__rp_operation.php"),
        ("DOCUMENT_ROOT", "/fixture"),
        ("QUERY_STRING", "rp_operation_cgi=1"),
        ("REQUEST_URI", "/__rp_operation.php?rp_operation_cgi=1"),
        ("CONTENT_LENGTH", "0"),
        (
            "RP_OPERATION_CASE",
            if index == 1 { "loop" } else { "normal" },
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect()
}

pub(crate) fn describe_cgi_fixture(report: &mut Value) {
    report["fixture"] = json!({"source":CGI_SOURCE,"guest_path":CGI_GUEST,
        "expected_sha256":CGI_FIXTURE_SHA,"verified":false,
        "canonical_source":"experiments/php-wasix-host-probe/src/php_operation_budget.rs",
        "case_bodies_byte_identical":false});
    report["isolation"]["guest_fixture_files"] = json!([CGI_GUEST]);
    report["isolation"]["guest_readonly_mounts"] = json!(["/fixture"]);
    report["isolation"]["guest_environment_supplied"] = Value::Null;
    report["isolation"]["guest_environment_policy"] =
        json!("fixed CGI scaffold and closed normal/loop case");
    report["isolation"]["asynchronous_threading"] = json!(false);
    report["isolation"]["deep_sleep"] = json!(false);
    report["isolation"]["max_guest_threads"] = json!(1);
}

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
    report["configured_initial_points"] = json!({"kind":"remaining","points":POINTS});
    report["budget_reset_count"] = json!(0);
    report["start_execution_included"] = json!(true);
    report["setup_observation_phase"] = json!("after-Instance.new-start-before-_initialize");
    report["fresh_post_start_points_equal"] = json!(false);
    report["post_start_reference_points"] = Value::Null;
    report["final_observer_strategy"] = json!(OBSERVER_STRATEGY);
    report["final_observation_phase"] =
        json!("original-recycle-store-after-guest-before-prior-recycle");
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
        "The compiled globals initialize each fresh instance to the fixed budget. Start-section execution spends it before setup observes the remainder; no points are reset.",
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

fn inspect_compiled_budget(module: &Module, report: &mut Value) -> Result<()> {
    let info = module.info();
    let start = info.start_function;
    report["module_start_section_present"] = json!(start.is_some());
    report["module_start_function"] = start.map_or(Value::Null, |index| {
        let exports: Vec<_> = info
            .exports
            .iter()
            .filter_map(|(name, export)| {
                matches!(export, ExportIndex::Function(value) if *value == index).then_some(name)
            })
            .collect();
        json!({"index":index.as_u32(),"name":info.function_names.get(&index),
            "exported_names":exports,"imported":info.is_imported_function(index)})
    });
    if let Some(index) = start {
        ensure!(
            !info.is_imported_function(index),
            "start must be a locally metered Wasm function"
        );
    }
    let mut globals = Vec::new();
    for (name, ty, expected) in [
        (
            "wasmer_metering_remaining_points",
            Type::I64,
            GlobalInit::I64Const(POINTS as i64),
        ),
        (
            "wasmer_metering_points_exhausted",
            Type::I32,
            GlobalInit::I32Const(0),
        ),
    ] {
        let index = match info.exports.get(name) {
            Some(ExportIndex::Global(index)) => *index,
            _ => anyhow::bail!("compiled metering export is not a global"),
        };
        ensure!(
            info.globals.get(index) == Some(&GlobalType::new(ty, Mutability::Var)),
            "compiled metering global has the wrong type"
        );
        let local = info
            .local_global_index(index)
            .context("metering initializer cannot be imported")?;
        let initializer = info
            .global_initializers
            .get(local)
            .context("missing compiled metering initializer")?;
        ensure!(
            *initializer == expected,
            "compiled metering initializer differs from the fixed constructor budget"
        );
        globals.push(json!({"export":name,"global_index":index.as_u32(),
            "type":if ty == Type::I64 {"mutable i64"} else {"mutable i32"},
            "initializer_kind":if ty == Type::I64 {"I64Const"} else {"I32Const"},
            "initializer_value":if ty == Type::I64 {POINTS} else {0}}));
    }
    report["compiled_metering_initializers"] = json!({"verified":true,"globals":globals,
        "scope":"public ModuleInfo initializers before instantiation/start; not a post-start observation"});
    Ok(())
}

fn post_start_remaining(points: &Value) -> Result<u64> {
    ensure!(
        points["kind"] == "remaining",
        "start did not leave a Remaining observation"
    );
    let remaining = points["points"]
        .as_u64()
        .context("post-start points are not an unsigned integer")?;
    ensure!(
        remaining > 0 && remaining <= POINTS,
        "post-start budget is exhausted or exceeds the compiled initial value"
    );
    Ok(remaining)
}

/// Only integer global handles are retained, never Instance/Env/Runtime/Store.
/// In the pinned sys backend, Global::clone copies a StoreHandle (store ID and
/// index). StoreObjects owns both VMGlobal and VMInstance until Store drop.
#[derive(Debug)]
struct MeteringGlobals {
    remaining: Global,
    exhausted: Global,
}

impl MeteringGlobals {
    fn capture(store: &mut impl AsStoreMut, instance: &Instance) -> Result<Self> {
        let globals = Self {
            remaining: instance
                .exports
                .get_global("wasmer_metering_remaining_points")
                .context("missing remaining-points global")?
                .clone(),
            exhausted: instance
                .exports
                .get_global("wasmer_metering_points_exhausted")
                .context("missing exhausted global")?
                .clone(),
        };
        globals.check_types(store)?;
        ensure!(
            globals.remaining != globals.exhausted,
            "metering globals alias"
        );
        Ok(globals)
    }

    fn same_store(&self, store: &impl AsStoreMut) -> bool {
        self.remaining.is_from_store(store) && self.exhausted.is_from_store(store)
    }

    fn check_types(&self, store: &impl AsStoreMut) -> Result<()> {
        // Check identity before invoking ty/get: sys StoreHandle::get asserts
        // its store ID. The recycle callback supplies the original owned Store.
        ensure!(
            self.same_store(store),
            "metering handles belong to another Store"
        );
        ensure!(
            self.remaining.ty(store) == GlobalType::new(Type::I64, Mutability::Var)
                && self.exhausted.ty(store) == GlobalType::new(Type::I32, Mutability::Var),
            "wrong captured metering global type"
        );
        Ok(())
    }

    fn identity(&self) -> String {
        // Pinned Global Debug contains StoreHandle's store ID/index, not values.
        // This is an opaque diagnostic binding; is_from_store is the real guard.
        sha(format!("{:?}\n{:?}", self.remaining, self.exhausted).as_bytes())
    }

    fn read(&self, store: &mut impl AsStoreMut) -> Result<Value> {
        self.check_types(store)?;
        let flag = match self.exhausted.get(store) {
            wasmer::Value::I32(flag) => flag,
            _ => anyhow::bail!("exhausted global did not return i32"),
        };
        ensure!(
            flag == 0 || flag == 1,
            "noncanonical metering exhausted flag"
        );
        if flag == 1 {
            return Ok(json!({"kind":"exhausted"}));
        }
        let points = match self.remaining.get(store) {
            // Wasmer's upstream getter interprets the i64 bit pattern as u64.
            wasmer::Value::I64(points) => points as u64,
            _ => anyhow::bail!("remaining-points global did not return i64"),
        };
        Ok(json!({"kind":"remaining","points":points}))
    }
}

#[derive(Debug, Default)]
struct Observations {
    setup_calls: usize,
    recycle_calls: usize,
    post_start: Option<Value>,
    final_points: Option<Value>,
    error: Option<String>,
    retained: Option<MeteringGlobals>,
    capture_count: usize,
    take_count: usize,
    drop_count: usize,
    setup_identity: Option<String>,
    recycle_identity: Option<String>,
    same_store_verified: bool,
    recycle_types_verified: bool,
    released_before_prior: bool,
    prior_present: bool,
    prior_calls: usize,
    prior_returns: usize,
}
type SharedObservations = Arc<Mutex<Observations>>;

fn lock_observations(state: &SharedObservations) -> MutexGuard<'_, Observations> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover only to release retained handles and chain prior cleanup.
            // Poisoning remains a latched diagnostic failure in the snapshot.
            let mut guard = poisoned.into_inner();
            guard
                .error
                .get_or_insert("metering observation mutex poisoned".into());
            guard
        }
    }
}

fn handle_lifecycle(state: &Observations) -> Value {
    json!({
        "strategy":OBSERVER_STRATEGY,
        "capture_count":state.capture_count,
        "take_count":state.take_count,
        "drop_count":state.drop_count,
        "retained_global_handles":if state.retained.is_some() {2} else {0},
        "setup_global_identity_sha256":state.setup_identity,
        "recycle_global_identity_sha256":state.recycle_identity,
        "identity_scheme":OBSERVER_IDENTITY,
        "same_store_verified":state.same_store_verified,
        "recycle_types_verified":state.recycle_types_verified,
        "released_before_prior_recycle":state.released_before_prior,
        "prior_recycle_present":state.prior_present,
        "prior_recycle_calls":state.prior_calls,
        "prior_recycle_returns":state.prior_returns,
        "instance_retained":false,
        "environment_lookup_at_recycle":false,
        "globals_reconstructed":false,
        "global_setter_calls":0
    })
}

fn observe_setup(
    state: &SharedObservations,
    store: &mut StoreMut,
    instance: &Instance,
) -> Result<()> {
    {
        let mut state = lock_observations(state);
        state.setup_calls = state.setup_calls.saturating_add(1);
        if state.setup_calls != 1 || state.retained.is_some() {
            state
                .error
                .get_or_insert("more than one instance setup callback".into());
            anyhow::bail!("more than one instance setup callback");
        }
    }
    let globals = MeteringGlobals::capture(store, instance);
    let mut state = lock_observations(state);
    match globals {
        Ok(globals) => {
            let observed = globals.read(store);
            state.setup_identity = Some(globals.identity());
            state.capture_count = state.capture_count.saturating_add(1);
            // The two store-index handles form no Env/Runtime/Instance cycle.
            // Recycle must take this exact bundle, read, then drop it once.
            state.retained = Some(globals);
            match observed {
                Ok(points) => {
                    // Instance::new already ran start; never replenish it.
                    state.post_start = Some(points.clone());
                    if let Err(error) = post_start_remaining(&points) {
                        state.error.get_or_insert(format!("{error:#}"));
                        return Err(error);
                    }
                    ensure!(state.error.is_none(), "metering setup observation failed");
                    Ok(())
                }
                Err(error) => {
                    state.error.get_or_insert(format!("{error:#}"));
                    Err(error)
                }
            }
        }
        Err(error) => {
            state.error.get_or_insert(format!("{error:#}"));
            Err(error)
        }
    }
}

fn observe_recycle(state: &SharedObservations, props: &mut TaskWasmRecycleProperties) {
    let globals = {
        let mut state = lock_observations(state);
        state.recycle_calls = state.recycle_calls.saturating_add(1);
        if state.recycle_calls != 1 {
            state
                .error
                .get_or_insert("more than one recycle callback".into());
        }
        let globals = state.retained.take();
        if globals.is_some() {
            state.take_count = state.take_count.saturating_add(1);
        }
        globals
    };
    let (observed, identity, same_store, types_verified, dropped) = match globals {
        Some(globals) => {
            let identity = globals.identity();
            let same_store = globals.same_store(&props.store);
            let types_verified = globals.check_types(&props.store).is_ok();
            let observed = globals.read(&mut props.store);
            // No Global handles are left in local/shared state when the prior
            // callback receives ownership of this original Store.
            drop(globals);
            (observed, Some(identity), same_store, types_verified, true)
        }
        None => (
            Err(anyhow::anyhow!(
                "recycle has no setup-captured metering globals"
            )),
            None,
            false,
            false,
            false,
        ),
    };
    let mut state = lock_observations(state);
    if dropped {
        state.drop_count = state.drop_count.saturating_add(1);
    }
    state.recycle_identity = identity;
    state.same_store_verified = same_store;
    state.recycle_types_verified = types_verified;
    state.released_before_prior = dropped && state.retained.is_none();
    if state.setup_identity != state.recycle_identity {
        state
            .error
            .get_or_insert("recycle global identity differs from setup".into());
    }
    match observed {
        Ok(points) => {
            if state.final_points.is_none() {
                state.final_points = Some(points);
            } else {
                state
                    .error
                    .get_or_insert("more than one final-points observation".into());
            }
        }
        Err(error) => {
            state.error.get_or_insert(format!("{error:#}"));
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
        if task.globals.is_some() || task.callbacks.trigger.is_some() {
            return Err(WasiThreadError::Unsupported);
        }
        let previous = task.callbacks.recycle.take();
        lock_observations(&self.observations).prior_present = previous.is_some();
        let observations = self.observations.clone();
        task.callbacks.recycle = Some(Box::new(move |mut props| {
            observe_recycle(&observations, &mut props);
            // Observation errors are latched, never an early return that skips
            // the original WASIX callback and its owned Store cleanup.
            if let Some(previous) = previous {
                {
                    let mut state = lock_observations(&observations);
                    state.prior_calls = state.prior_calls.saturating_add(1);
                }
                previous(props);
                let mut state = lock_observations(&observations);
                state.prior_returns = state.prior_returns.saturating_add(1);
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
    verify_profile(bytes, report, EntryProfile::Cli)
}

pub(crate) fn verify_cgi(bytes: Vec<u8>, report: &mut Value) -> Result<()> {
    ensure!(
        sha(CGI_FIXTURE) == CGI_FIXTURE_SHA,
        "fixed operation CGI fixture identity"
    );
    for body in [NORMAL, LOOP] {
        ensure!(
            CGI_FIXTURE
                .windows(body.len())
                .filter(|window| *window == body.as_bytes())
                .count()
                == 1,
            "original operation body must occur exactly once in CGI fixture"
        );
    }
    report["fixture"]["verified"] = json!(true);
    report["fixture"]["observed_sha256"] = json!(sha(CGI_FIXTURE));
    report["fixture"]["case_bodies_byte_identical"] = json!(true);
    verify_profile(bytes, report, EntryProfile::ManagedCgi)
}

fn verify_profile(bytes: Vec<u8>, report: &mut Value, profile: EntryProfile) -> Result<()> {
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
    inspect_compiled_budget(&module, report)?;
    let mut all_met = true;
    let mut reference = None;
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
            "configured_initial_points":{"kind":"remaining","points":POINTS},"budget_reset_count":0,
            "setup_observation_phase":"after-Instance.new-start-before-_initialize",
            "post_start_points":null,"start_charged_points":null,"post_start_difference_from_first":null,
            "post_start_matches_first":false,"final_points":null,"setup_observation_count":0,"recycle_observation_count":0,
            "metering_handle_lifecycle":handle_lifecycle(&Observations::default()),
            "metering_observation_error":null,"ready_reached":false,"budget_exhausted":false,"php":null,
            "guest_environment_supplied":[],"host_environment_forwarded":false,"host_directories_mapped":false,
            "home_mapped":false,"networking":"UnsupportedVirtualNetworking","http_client":false,
            "stdin":"closed-pipe","stdout_limit_bytes":OUTPUT_LIMIT,"stderr_limit_bytes":OUTPUT_LIMIT});
        if profile == EntryProfile::ManagedCgi {
            record["guest_arguments"] = json!([
                "-n",
                "-d",
                "fiber.stack_size=2097152",
                "-d",
                "register_argc_argv=0"
            ]);
            record["guest_environment_supplied"] = json!(cgi_environment(index));
            record["program_sha256"] = json!(sha(CGI_FIXTURE));
            record["fresh_wasi_runtime"] = json!(true);
            record["complete_cgi_response_verified"] = json!(false);
        }
        let execution = execute(&engine, module.clone(), &data, index, &mut record, profile);
        let observed = post_start_remaining(&record["post_start_points"]).ok();
        if index == 0 {
            reference = observed;
            report["post_start_reference_points"] = json!(reference);
        }
        if let (Some(points), Some(first)) = (observed, reference) {
            record["post_start_difference_from_first"] = json!(points as i64 - first as i64);
            record["post_start_matches_first"] = json!(points == first);
        }
        let result = execution.and_then(|()| validate(&record, index, profile));
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
    report["fresh_post_start_points_equal"] =
        json!(report["cases"].as_array().is_some_and(|cases| {
            cases.len() == 3
                && cases
                    .iter()
                    .all(|case| case["post_start_matches_first"] == true)
        }));
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
    profile: EntryProfile,
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
        if profile == EntryProfile::Cli {
            runner.with_args(["-n", "-r", if index == 1 { LOOP } else { NORMAL }]);
        } else {
            runner
                .with_args([
                    "-n",
                    "-d",
                    "fiber.stack_size=2097152",
                    "-d",
                    "register_argc_argv=0",
                ])
                .with_envs(cgi_environment(index))
                .with_current_dir("/fixture")
                .with_mount(
                    "/fixture".into(),
                    Arc::new(super::language_contract::single_file_fs(
                        CGI_FILENAME,
                        CGI_FIXTURE,
                    )),
                );
        }
        runner
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
        let observation = lock_observations(&observations);
        record["setup_observation_count"] = json!(observation.setup_calls);
        record["recycle_observation_count"] = json!(observation.recycle_calls);
        record["post_start_points"] = json!(observation.post_start);
        if let Some(points) = &observation.post_start {
            if let Ok(remaining) = post_start_remaining(points) {
                record["start_charged_points"] = json!(POINTS - remaining);
            }
        }
        record["final_points"] = json!(observation.final_points);
        record["metering_observation_error"] = json!(observation.error);
        record["metering_handle_lifecycle"] = handle_lifecycle(&observation);
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
        let body = if profile == EntryProfile::ManagedCgi {
            // Only normal successful requests can claim a complete CGI response.
            // The exhausted loop's direct php://stdout READY remains raw data.
            let response = super::cgi_contract::cgi_response::parse(&stdout)?;
            record["response"] = json!({"status":response.status,
                "headers":response.headers.iter().map(|(name,value)| json!({"name":name,"value_hex":hex(value)})).collect::<Vec<_>>(),
                "body_bytes":response.body.len(),"body_hex":hex(&response.body),
                "body_sha256":sha(&response.body),"representation_length":response.representation_length});
            ensure!(response.status == 200, "normal operation CGI status");
            record["complete_cgi_response_verified"] = json!(true);
            response.body
        } else {
            stdout
        };
        record["php"] =
            serde_json::from_slice::<Value>(&body).context("normal PHP operation-budget JSON")?;
    }
    // Expected guest errors are preserved above and classified only by validate,
    // which requires READY plus the independently observed exhausted global.
    Ok(())
}

fn validate_handle_lifecycle(record: &Value) -> Result<()> {
    let lifecycle = &record["metering_handle_lifecycle"];
    ensure!(
        lifecycle["strategy"] == OBSERVER_STRATEGY
            && lifecycle["identity_scheme"] == OBSERVER_IDENTITY
            && lifecycle["capture_count"] == 1
            && lifecycle["take_count"] == 1
            && lifecycle["drop_count"] == 1
            && lifecycle["retained_global_handles"] == 0
            && lifecycle["same_store_verified"] == true
            && lifecycle["recycle_types_verified"] == true
            && lifecycle["released_before_prior_recycle"] == true
            && lifecycle["instance_retained"] == false
            && lifecycle["environment_lookup_at_recycle"] == false
            && lifecycle["globals_reconstructed"] == false
            && lifecycle["global_setter_calls"] == 0,
        "captured metering globals were missing, reused, cross-store or not released"
    );
    let identity = lifecycle["setup_global_identity_sha256"]
        .as_str()
        .context("missing setup-global identity")?;
    ensure!(
        identity.len() == 64
            && identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            && lifecycle["recycle_global_identity_sha256"] == identity,
        "setup/recycle global-handle identities differ"
    );
    let prior = lifecycle["prior_recycle_present"]
        .as_bool()
        .context("missing prior-recycle presence")?;
    let expected = if prior { 1 } else { 0 };
    ensure!(
        lifecycle["prior_recycle_calls"] == expected
            && lifecycle["prior_recycle_returns"] == expected,
        "prior recycle callback was not chained exactly once"
    );
    Ok(())
}

fn validate(record: &Value, index: usize, profile: EntryProfile) -> Result<()> {
    validate_handle_lifecycle(record)?;
    ensure!(
        record["setup_observation_count"] == 1
            && record["recycle_observation_count"] == 1
            && record["metering_observation_error"].is_null(),
        "missing/repeated/failed safe metering observations"
    );
    let post_start = post_start_remaining(&record["post_start_points"])?;
    ensure!(
        record["configured_initial_points"] == json!({"kind":"remaining","points":POINTS})
            && record["budget_reset_count"] == 0
            && record["start_charged_points"] == POINTS - post_start,
        "configured budget, start charge or no-reset record mismatch"
    );
    ensure!(
        record["post_start_matches_first"] == true
            && record["post_start_difference_from_first"] == 0,
        "fresh instances differ in their observed post-start budget"
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
            record["final_points"]["kind"] == "remaining" && points > 0 && points < post_start,
            "normal case did not consume points after start and leave a positive remainder"
        );
        ensure!(
            record["php"]
                == json!({"php_version":"8.5.12","version_id":80512,"php_int_size":8,
            "sapi":if profile == EntryProfile::Cli { "cli" } else { "cgi-fcgi" },
            "kind":"normal","globals_before_present":false,"globals_after":"request-local",
            "echo":"recastpress-v0.5-php-operation-budget"}),
            "normal PHP response differs from the fixed contract"
        );
    }
    Ok(())
}

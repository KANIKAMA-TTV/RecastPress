//! Closed WAT-only metering observations, not PHP or wall-clock CPU limits.
//!
//! Run the test executable under an owned finite-deadline supervisor: a broken
//! compiler/middleware must not turn the infinite-loop diagnostic into a hang.
//! A new Metering/Engine is created for each module; that middleware must never
//! be reused to compile a different module. Instances reuse one compiled Module.

use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use wasmer::sys::CompilerConfig;
use wasmer::wasmparser::Operator;
use wasmer::{Engine, Instance, Module, Store, TypedFunction, imports};
use wasmer_middlewares::{
    Metering,
    metering::{MeteringPoints, get_remaining_points},
};

const INITIAL_POINTS: u64 = 10_000;
const PREFIX: &str = "P0_METERING_JSON=";
const CASES: [&str; 3] = ["normal_first", "infinite_loop", "normal_recovery"];

const PLAIN_WAT: &str = r#"
(module
  (global $entered (export "entered") (mut i32) (i32.const 0))
  (func (export "normal") (result i32)
    i32.const 41
    i32.const 1
    i32.add)
  (func (export "spin")
    i32.const 1
    global.set $entered
    (loop $forever
      br $forever)))
"#;

// Formal EH only: no legacy try/catch opcodes. The normal path must receive the
// thrown payload at the catch target. The loop is reached through an inner
// catch_all and remains inside an outer try_table. If the outer catch returns,
// it changes `entered` to 2, which is a failure rather than an accepted timeout.
#[cfg(all(target_os = "linux", not(target_env = "msvc")))]
const FORMAL_EH_WAT: &str = r#"
(module
  (tag $event (param i32))
  (global $entered (export "entered") (mut i32) (i32.const 0))
  (func (export "normal") (result i32)
    (block $caught (result i32)
      (try_table (catch $event $caught)
        i32.const 41
        throw $event)
      unreachable)
    i32.const 1
    i32.add)
  (func (export "spin")
    (block $unexpected_catch
      (try_table (catch_all $unexpected_catch)
        (block $caught
          (try_table (catch_all $caught)
            i32.const 7
            throw $event)
          unreachable)
        i32.const 1
        global.set $entered
        (loop $forever
          br $forever)))
    i32.const 2
    global.set $entered))
"#;

fn emit(value: Value) -> Result<()> {
    // A leading newline also keeps the prefix separate from libtest's own
    // progress text when an owned runner opts into --nocapture/--show-output.
    println!("\n{PREFIX}{value}");
    std::io::stdout().flush().context("flush WAT observation")
}

fn points_value(points: &MeteringPoints) -> Value {
    match points {
        MeteringPoints::Remaining(points) => json!({"kind": "remaining", "points": points}),
        MeteringPoints::Exhausted => json!({"kind": "exhausted"}),
    }
}

fn compile_once(source: &str) -> Result<(Engine, Module)> {
    // Cost is chosen at compile time. These points count weighted Wasm
    // operators, not native instructions, PHP opcodes, CPU seconds or host IO.
    let metering = Arc::new(Metering::new(INITIAL_POINTS, |_operator: &Operator| 1));
    #[cfg(feature = "llvm")]
    let mut compiler = wasmer_compiler_llvm::LLVM::default();
    #[cfg(all(not(feature = "llvm"), feature = "cranelift"))]
    let mut compiler = wasmer_compiler_cranelift::Cranelift::default();
    compiler.push_middleware(metering);
    let engine: Engine = compiler.into();
    let bytes = wat::parse_str(source).context("parse fixed metering WAT")?;
    // This is the sole module compile call for this test's three instances.
    let module = Module::new(&engine, bytes).context("compile metered WAT once")?;
    ensure!(
        module.info().start_function.is_none(),
        "unexpected WAT start section"
    );
    ensure!(
        module.imports().next().is_none(),
        "WAT must have no hostcall imports"
    );
    Ok((engine, module))
}

fn verify_three_instances(source: &str, test_name: &str, formal_eh: bool) -> Result<()> {
    let (engine, module) = compile_once(source)?;
    let mut observations = Vec::new();
    for (index, case) in CASES.iter().enumerate() {
        // Fresh stores and instances ensure the exhausted global from the
        // second instance cannot be reused or mistaken for a reset in place.
        let mut store = Store::new(engine.clone());
        let instance = Instance::new(&mut store, &module, &imports! {})
            .context("instantiate the same metered module in a fresh store")?;
        let initial = get_remaining_points(&mut store, &instance);
        ensure!(
            initial == MeteringPoints::Remaining(INITIAL_POINTS),
            "fresh instance did not restore the exact budget"
        );
        let entered = instance.exports.get_global("entered")?;
        ensure!(
            entered.get(&mut store).i32() == Some(0),
            "fresh instance inherited guest state"
        );
        emit(json!({
            "schema": "recastpress.wasmer-metering-wat.v0.5", "event": "case_start",
            "test": test_name, "engine": super::ENGINE_NAME,
            "case": case, "instance_number": index + 1,
            "configured_budget": INITIAL_POINTS, "initial_points": points_value(&initial),
            "formal_eh": formal_eh, "php_executed": false
        }))?;
        let mut observation = json!({
            "case": case, "instance_number": index + 1,
            "initial_points": points_value(&initial), "normal_result": null,
            "call_failed": false, "runtime_trap_observed": false
        });
        if index == 1 {
            let spin: TypedFunction<(), ()> =
                instance.exports.get_typed_function(&store, "spin")?;
            let error = spin
                .call(&mut store)
                .expect_err("infinite WAT loop must be interrupted");
            let trap_observed = error.to_trap().is_some();
            let remaining = get_remaining_points(&mut store, &instance);
            observation["call_failed"] = json!(true);
            observation["runtime_trap_observed"] = json!(trap_observed);
            observation["final_points"] = points_value(&remaining);
            observation["entered"] = json!(entered.get(&mut store).i32());
            ensure!(trap_observed, "loop failed without an actual Wasmer trap");
            ensure!(
                remaining == MeteringPoints::Exhausted,
                "loop failure was not actual metering exhaustion"
            );
            ensure!(
                entered.get(&mut store).i32() == Some(1),
                "loop/catch path was not entered or the outer catch swallowed the trap"
            );
        } else {
            let normal: TypedFunction<(), i32> =
                instance.exports.get_typed_function(&store, "normal")?;
            let result = normal
                .call(&mut store)
                .context("run finite normal WAT path")?;
            let remaining = get_remaining_points(&mut store, &instance);
            observation["normal_result"] = json!(result);
            observation["final_points"] = points_value(&remaining);
            observation["entered"] = json!(entered.get(&mut store).i32());
            ensure!(result == 42, "normal/catch payload result mismatch");
            ensure!(
                matches!(remaining, MeteringPoints::Remaining(points) if points > 0 && points < INITIAL_POINTS),
                "normal WAT must consume points and leave a positive remainder"
            );
            ensure!(
                entered.get(&mut store).i32() == Some(0),
                "normal instance inherited loop state"
            );
        }
        observation["verified"] = json!(true);
        emit(json!({
            "schema": "recastpress.wasmer-metering-wat.v0.5", "event": "case_result",
            "test": test_name, "engine": super::ENGINE_NAME,
            "formal_eh": formal_eh, "php_executed": false, "observation": observation
        }))?;
        observations.push(observation);
    }
    emit(json!({
        "schema": "recastpress.wasmer-metering-wat.v0.5", "event": "test_complete",
        "test": test_name, "engine": super::ENGINE_NAME, "ok": true,
        "scope": "fixed-WAT-only-one-compile-three-fresh-instances",
        "configured_budget": INITIAL_POINTS, "cost_policy": "one-point-per-original-Wasm-operator",
        "explicit_module_compile_calls": 1, "fresh_instances": 3,
        "normal_recovery_verified": true, "formal_eh": formal_eh,
        "php_executed": false, "hostcall_timeout_verified": false,
        "cpu_wall_clock_deadline_verified": false, "observations": observations
    }))
}

#[test]
fn one_metered_module_exhausts_a_loop_and_fresh_instances_restore_the_budget() -> Result<()> {
    verify_three_instances(PLAIN_WAT, "plain", false)
}

// MSVC formal exception entry points are unimplemented in Wasmer 7.2.0. This
// test is compiled for Linux LLVM and Cranelift candidates. It is not compiled
// for Windows/MSVC; its absence there means formal EH remains unexecuted.
#[cfg(all(target_os = "linux", not(target_env = "msvc")))]
#[test]
fn linux_formal_eh_catch_loop_exhausts_and_a_fresh_instance_recovers() -> Result<()> {
    verify_three_instances(FORMAL_EH_WAT, "formal_eh", true)
}

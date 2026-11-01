//! Fixed UTF-8 Component Model feasibility observations, not an adopted SDK,
//! WIT, ABI, engine, full sandbox, capability policy or public Wasm runner.
//! No arbitrary inputs, WASI/host definitions, async/cache, PHP, DB or HTTP.
//! Fuel excludes compilation/wall-time; limits exclude engine/compiler/host
//! memory, whole RSS/capacity/concurrency. UTF-8 WasmStr borrowing avoids an
//! owned host string before the copy cap only for these fixed UTF-8 fixtures.
#![forbid(unsafe_code)]
use std::{borrow::Cow, fmt, str::Utf8Error};
use wasmtime::component::{Component, Instance, Linker, WasmStr};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap};

const FUEL: u64 = 10_000;
const PAGE: usize = 65_536;
const LITERAL: &[u8] = "fixed component output\0é".as_bytes();
const PAYLOAD: &str = r"fixed component output\00\c3\a9";
const RECORD: &str = r"\20\00\00\00\19\00\00\00";
const RENDER: &str = "i32.const 32 i32.const 102 i32.store8 i32.const 8";
const POST: &str = "local.get 0 i32.const 8 i32.ne if unreachable end global.get $posts i32.const 1 i32.add global.set $posts i32.const 32 i32.const 88 i32.store8";
const SPIN: &str = "(loop $spin br $spin)";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Engine,
    Compile,
    Instantiate,
    ExportMismatch,
    ImportRefused,
    RecordAlignment,
    RecordBounds,
    StringBounds,
    InvalidUtf8,
    OutputLimit,
    Allocation,
    FuelExhausted,
    CannotEnter,
    GuestTrap,
    PostReturnFuel,
    PostReturnTrap,
    UnexpectedObservation,
}
impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only fixed variant names, without provider diagnostics or payload.
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProbeError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observations {
    pub fixture_checks: usize,
    pub literal_output_bytes: usize,
    pub fuel_per_store: u64,
}

fn engine() -> Result<Engine, ProbeError> {
    let mut configuration = Config::new();
    configuration.consume_fuel(true).wasm_component_model(true);
    Engine::new(&configuration).map_err(|_| ProbeError::Engine)
}
fn new_store(
    engine: &Engine,
    instances: usize,
    memories: usize,
    tables: usize,
) -> Result<Store<StoreLimits>, ProbeError> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(PAGE)
        .table_elements(16)
        .instances(instances)
        .memories(memories)
        .tables(tables)
        .build();
    let mut store = Store::new(engine, limits);
    // Install before instantiate; core start functions are included.
    store.limiter(|limits| limits);
    store.set_fuel(FUEL).map_err(|_| ProbeError::Engine)?;
    Ok(store)
}
fn fixture(render: &str, post: &str, record: &str, payload: &str, extras: &str) -> String {
    // Only fixed source literals reach this private builder, not user input.
    format!(
        r#"(component
      (core module $m
        (memory (export "memory") 1)
        (global $posts (mut i32) (i32.const 0))
        (global $calls (mut i32) (i32.const 0))
        (data (i32.const 8) "{record}") (data (i32.const 32) "{payload}")
        (func (export "render") (result i32) {render})
        (func (export "post") (param i32) {post})
        (func (export "post-count") (result i32) global.get $posts)
        {extras})
      (core instance $i (instantiate $m))
      (func (export "render") (result string)
        (canon lift (core func $i "render") (memory (core memory $i "memory")) (post-return (core func $i "post"))))
      (func (export "post-count") (result u32) (canon lift (core func $i "post-count"))))"#
    )
}
fn compile_component(engine: &Engine, source: &str) -> Result<Component, ProbeError> {
    Component::new(engine, source).map_err(|_| ProbeError::Compile)
}
fn standard(engine: &Engine) -> Result<Component, ProbeError> {
    compile_component(engine, &fixture(RENDER, POST, RECORD, PAYLOAD, ""))
}
fn cause(error: &wasmtime::Error, expected: &str) -> bool {
    error.chain().any(|item| item.to_string() == expected)
}
fn classify(error: &wasmtime::Error) -> ProbeError {
    match error.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel) => ProbeError::FuelExhausted,
        Some(Trap::CannotEnterComponent) => ProbeError::CannotEnter,
        Some(_) => ProbeError::GuestTrap,
        None if cause(error, "return pointer not aligned") => ProbeError::RecordAlignment,
        None if cause(error, "pointer out of bounds of memory") => ProbeError::RecordBounds,
        None if cause(error, "string pointer/length out of bounds of memory") => {
            ProbeError::StringBounds
        }
        None => ProbeError::Instantiate,
    }
}
fn instantiate(
    engine: &Engine,
    component: &Component,
) -> Result<(Store<StoreLimits>, Instance), ProbeError> {
    let mut store = new_store(engine, 1, 1, 1)?;
    let instance = Linker::new(engine)
        .instantiate(&mut store, component)
        .map_err(|error| classify(&error))?;
    Ok((store, instance))
}
fn copy(
    value: &WasmStr,
    store: &Store<StoreLimits>,
    maximum: usize,
) -> Result<Vec<u8>, ProbeError> {
    let text = value.to_str(store).map_err(|error| {
        if error.downcast_ref::<Utf8Error>().is_some() {
            ProbeError::InvalidUtf8
        } else {
            ProbeError::UnexpectedObservation
        }
    })?;
    if !matches!(text, Cow::Borrowed(_)) {
        return Err(ProbeError::UnexpectedObservation);
    }
    if text.len() > maximum {
        return Err(ProbeError::OutputLimit);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(text.len())
        .map_err(|_| ProbeError::Allocation)?;
    output.extend_from_slice(text.as_bytes());
    Ok(output)
}
fn render(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    maximum: usize,
) -> Result<Vec<u8>, ProbeError> {
    let function = instance
        .get_typed_func::<(), (WasmStr,)>(&mut *store, "render")
        .map_err(|_| ProbeError::ExportMismatch)?;
    // Err from call has no post_return obligation. Discard this Store.
    let (value,) = function
        .call(&mut *store, ())
        .map_err(|error| classify(&error))?;
    let tentative = copy(&value, store, maximum); // All borrows end here.
    // Exactly once after EVERY successful call, even host validation refusal.
    // Cleanup failure is terminal and overrides any tentative output/error.
    function.post_return(&mut *store).map_err(|error| {
        if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
            ProbeError::PostReturnFuel
        } else {
            ProbeError::PostReturnTrap
        }
    })?;
    tentative
}
fn post_count(store: &mut Store<StoreLimits>, instance: &Instance) -> Result<u32, ProbeError> {
    let function = instance
        .get_typed_func::<(), (u32,)>(&mut *store, "post-count")
        .map_err(|_| ProbeError::ExportMismatch)?;
    let (value,) = function
        .call(&mut *store, ())
        .map_err(|error| classify(&error))?;
    // Mandatory even with no canonical post-return function on this export.
    function
        .post_return(&mut *store)
        .map_err(|_| ProbeError::PostReturnTrap)?;
    Ok(value)
}
fn require(value: bool) -> Result<(), ProbeError> {
    if value {
        Ok(())
    } else {
        Err(ProbeError::UnexpectedObservation)
    }
}
fn lifecycle_checks(engine: &Engine) -> Result<usize, ProbeError> {
    let component = standard(engine)?;
    let resources = component
        .resources_required()
        .ok_or(ProbeError::UnexpectedObservation)?;
    require(
        resources.num_memories == 1
            && resources.num_tables == 0
            && resources.max_initial_memory_size == Some(1)
            && resources.max_initial_table_size.is_none(),
    )?;
    let (mut store, instance) = instantiate(engine, &component)?;
    require(render(&mut store, &instance, LITERAL.len())? == LITERAL)?;
    require(post_count(&mut store, &instance)? == 1)?;
    require(render(&mut store, &instance, LITERAL.len())? == LITERAL)?;
    require(post_count(&mut store, &instance)? == 2)?;
    require(render(&mut store, &instance, LITERAL.len() - 1) == Err(ProbeError::OutputLimit))?;
    require(post_count(&mut store, &instance)? == 3)?;
    require(render(&mut store, &instance, LITERAL.len())? == LITERAL)?;
    Ok(8)
}
fn canonical_checks(engine: &Engine) -> Result<usize, ProbeError> {
    for (render_body, record, expected) in [
        ("i32.const 9", RECORD, ProbeError::RecordAlignment),
        ("i32.const 65532", RECORD, ProbeError::RecordBounds),
        (
            "i32.const 8",
            r"\00\00\01\00\01\00\00\00",
            ProbeError::StringBounds,
        ),
    ] {
        let component =
            compile_component(engine, &fixture(render_body, POST, record, PAYLOAD, ""))?;
        let (mut store, instance) = instantiate(engine, &component)?;
        require(render(&mut store, &instance, usize::MAX) == Err(expected))?;
    }
    let invalid = compile_component(
        engine,
        &fixture("i32.const 8", POST, r"\20\00\00\00\01\00\00\00", r"\ff", ""),
    )?;
    let (mut store, instance) = instantiate(engine, &invalid)?;
    require(render(&mut store, &instance, 1) == Err(ProbeError::InvalidUtf8))?;
    require(post_count(&mut store, &instance)? == 1)?;
    let component = standard(engine)?;
    let (mut store, instance) = instantiate(engine, &component)?;
    require(
        instance
            .get_typed_func::<(), (u32,)>(&mut store, "render")
            .is_err(),
    )?;
    require(
        instance
            .get_typed_func::<(), (WasmStr,)>(&mut store, "missing")
            .is_err(),
    )?;
    let imported = compile_component(
        engine,
        r#"(component (import "forbidden" (func (result string))))"#,
    )?;
    let mut store = new_store(engine, 1, 1, 1)?;
    let error = Linker::new(engine)
        .instantiate(&mut store, &imported)
        .err()
        .ok_or(ProbeError::UnexpectedObservation)?;
    require(cause(
        &error,
        "component imports function `forbidden`, but a matching implementation was not found in the linker",
    ))?;
    Ok(8)
}
fn failure_checks(engine: &Engine) -> Result<usize, ProbeError> {
    let component = standard(engine)?;
    let (mut store, instance) = instantiate(engine, &component)?;
    let function = instance
        .get_typed_func::<(), (WasmStr,)>(&mut store, "render")
        .map_err(|_| ProbeError::ExportMismatch)?;
    let _pending = function
        .call(&mut store, ())
        .map_err(|error| classify(&error))?;
    let error = function
        .call(&mut store, ())
        .err()
        .ok_or(ProbeError::UnexpectedObservation)?;
    require(error.downcast_ref::<Trap>() == Some(&Trap::CannotEnterComponent))?;
    // Discard pending Store; no double/wrong-function post_return panic tests.
    let start = compile_component(
        engine,
        &fixture(
            RENDER,
            POST,
            RECORD,
            PAYLOAD,
            "(func $start (loop $spin br $spin)) (start $start)",
        ),
    )?;
    require(instantiate(engine, &start).err() == Some(ProbeError::FuelExhausted))?;
    let loop_render = compile_component(
        engine,
        &fixture(&format!("{SPIN} unreachable"), POST, RECORD, PAYLOAD, ""),
    )?;
    let (mut loop_store, loop_instance) = instantiate(engine, &loop_render)?;
    require(render(&mut loop_store, &loop_instance, 0) == Err(ProbeError::FuelExhausted))?;
    for (body, record, payload, maximum) in [
        (RENDER, RECORD, PAYLOAD, 25),
        ("i32.const 8", r"\20\00\00\00\01\00\00\00", r"\ff", 1),
        (RENDER, RECORD, PAYLOAD, 0),
    ] {
        let component = compile_component(engine, &fixture(body, SPIN, record, payload, ""))?;
        let (mut store, instance) = instantiate(engine, &component)?;
        require(render(&mut store, &instance, maximum) == Err(ProbeError::PostReturnFuel))?;
    }
    Ok(6)
}
fn state_checks(engine: &Engine) -> Result<usize, ProbeError> {
    let body = "global.get $calls i32.const 1 i32.add global.set $calls i32.const 32 global.get $calls i32.const 48 i32.add i32.store8 i32.const 8";
    let component = compile_component(
        engine,
        &fixture(body, POST, r"\20\00\00\00\01\00\00\00", "1", ""),
    )?;
    let (mut store, instance) = instantiate(engine, &component)?;
    require(render(&mut store, &instance, 1)? == b"1")?;
    require(render(&mut store, &instance, 1)? == b"2")?;
    for _ in 0..2 {
        let (mut fresh, instance) = instantiate(engine, &component)?;
        require(render(&mut fresh, &instance, 1)? == b"1")?;
    }
    Ok(4)
}
fn resource_checks(engine: &Engine) -> Result<usize, ProbeError> {
    for (wat, instances, memories, tables, reason) in [
        (
            "(component (core module $m (memory 2)) (core instance (instantiate $m)))",
            1,
            1,
            1,
            "memory minimum size of 2 pages exceeds memory limits",
        ),
        (
            "(component (core module $m (table 17 funcref)) (core instance (instantiate $m)))",
            1,
            1,
            1,
            "table minimum size of 17 elements exceeds table limits",
        ),
        (
            "(component (core module $m (memory 1)) (core instance (instantiate $m)))",
            1,
            0,
            1,
            "resource limit exceeded: memory count too high at 1",
        ),
        (
            "(component (core module $m (table 1 funcref)) (core instance (instantiate $m)))",
            1,
            1,
            0,
            "resource limit exceeded: table count too high at 1",
        ),
        (
            "(component (core module $m) (core instance (instantiate $m)))",
            0,
            1,
            1,
            "resource limit exceeded: instance count too high at 1",
        ),
    ] {
        let component = compile_component(engine, wat)?;
        let mut store = new_store(engine, instances, memories, tables)?;
        let error = Linker::new(engine)
            .instantiate(&mut store, &component)
            .err()
            .ok_or(ProbeError::UnexpectedObservation)?;
        require(cause(&error, reason))?;
    }
    Ok(5)
}
/// Only fixed owned Component WAT can be reached from this public observation.
pub fn verify_fixed_fixtures() -> Result<Observations, ProbeError> {
    let engine = engine()?;
    let fixture_checks = lifecycle_checks(&engine)?
        + canonical_checks(&engine)?
        + failure_checks(&engine)?
        + state_checks(&engine)?
        + resource_checks(&engine)?;
    Ok(Observations {
        fixture_checks,
        literal_output_bytes: LITERAL.len(),
        fuel_per_store: FUEL,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_owned_copy_post_return_and_cap_refusal_cleanup() {
        assert_eq!(lifecycle_checks(&engine().unwrap()), Ok(8));
    }
    #[test]
    fn canonical_record_string_export_import_refusals_have_specific_reasons() {
        assert_eq!(canonical_checks(&engine().unwrap()), Ok(8));
    }
    #[test]
    fn missing_post_return_and_start_render_cleanup_fuel_fail_terminally() {
        assert_eq!(failure_checks(&engine().unwrap()), Ok(6));
    }
    #[test]
    fn same_instance_progress_and_fresh_store_reset() {
        assert_eq!(state_checks(&engine().unwrap()), Ok(4));
    }
    #[test]
    fn component_core_resources_are_limited_before_instantiation() {
        assert_eq!(resource_checks(&engine().unwrap()), Ok(5));
    }
}

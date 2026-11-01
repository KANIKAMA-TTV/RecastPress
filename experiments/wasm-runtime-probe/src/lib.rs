//! Unconnected Wasmtime 36.0.19 fixed-fixture capability experiment.
//! Not a selected engine, Component SDK, ABI or sandbox assurance. No WASI
//! dependency, host imports, cache, async, file input, network, PHP, DB or HTTP.
//! Fuel covers guest execution, not compilation/wall time. StoreLimits bound
//! per-memory/table and Store resource counts, not all RSS/compiler/engine/host
//! allocation/capacity/concurrency. Safe host APIs; provider internals use unsafe.
#![forbid(unsafe_code)]

use std::fmt;
use wasmtime::{
    Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Trap,
    UnknownImportError,
};
const FUEL: u64 = 10_000;
const PAGE: usize = 65_536;
const LITERAL: &str = r#"(module (memory (export "memory") 1)
 (data (i32.const 32) "fixed wasm output\00\ff")
 (func (export "render") (result i32 i32) i32.const 32 i32.const 19))"#;
const EXPECTED: &[u8] = b"fixed wasm output\0\xff";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Engine,
    Compile,
    Instantiate,
    ImportRefused,
    FuelExhausted,
    GuestTrap,
    ExportMismatch,
    NegativeOutput,
    OutputOutOfBounds,
    OutputLimit,
    Allocation,
    UnexpectedObservation,
}
impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Engine => "engine configuration",
            Self::Compile => "fixed fixture compilation",
            Self::Instantiate => "fixed fixture instantiation",
            Self::ImportRefused => "undefined fixture import",
            Self::FuelExhausted => "guest fuel exhausted",
            Self::GuestTrap => "guest trapped",
            Self::ExportMismatch => "fixed export shape mismatch",
            Self::NegativeOutput => "negative output range",
            Self::OutputOutOfBounds => "output outside guest memory",
            Self::OutputLimit => "host output limit",
            Self::Allocation => "host output allocation",
            Self::UnexpectedObservation => "unexpected fixed fixture observation",
        })
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
    let mut config = Config::new();
    config.consume_fuel(true);
    Engine::new(&config).map_err(|_| ProbeError::Engine)
}
fn store(engine: &Engine, instances: usize) -> Result<Store<StoreLimits>, ProbeError> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(PAGE)
        .table_elements(16)
        .instances(instances)
        .memories(1)
        .tables(1)
        .build();
    let mut store = Store::new(engine, limits);
    store.limiter(|limits| limits);
    // Installed before instantiate, including any start function.
    store.set_fuel(FUEL).map_err(|_| ProbeError::Engine)?;
    Ok(store)
}
fn classify(error: &wasmtime::Error) -> ProbeError {
    if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
        ProbeError::FuelExhausted
    } else if error.downcast_ref::<UnknownImportError>().is_some() {
        ProbeError::ImportRefused
    } else if error.downcast_ref::<Trap>().is_some() {
        ProbeError::GuestTrap
    } else {
        ProbeError::Instantiate
    }
}
fn module(engine: &Engine, wat: &str) -> Result<Module, ProbeError> {
    Module::new(engine, wat).map_err(|_| ProbeError::Compile)
}
fn range(
    pointer: i32,
    length: i32,
    memory: usize,
    cap: usize,
) -> Result<(usize, usize), ProbeError> {
    let pointer = usize::try_from(pointer).map_err(|_| ProbeError::NegativeOutput)?;
    let length = usize::try_from(length).map_err(|_| ProbeError::NegativeOutput)?;
    let end = pointer
        .checked_add(length)
        .ok_or(ProbeError::OutputOutOfBounds)?;
    if end > memory {
        return Err(ProbeError::OutputOutOfBounds);
    }
    if length > cap {
        return Err(ProbeError::OutputLimit);
    }
    Ok((pointer, length))
}
fn render(engine: &Engine, module: &Module, cap: usize) -> Result<Vec<u8>, ProbeError> {
    let mut store = store(engine, 1)?;
    let linker = Linker::new(engine); // No definitions or host capabilities.
    let instance = linker
        .instantiate(&mut store, module)
        .map_err(|error| classify(&error))?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(ProbeError::ExportMismatch)?;
    let function = instance
        .get_typed_func::<(), (i32, i32)>(&mut store, "render")
        .map_err(|_| ProbeError::ExportMismatch)?;
    let (pointer, length) = function
        .call(&mut store, ())
        .map_err(|error| classify(&error))?;
    let (pointer, length) = range(pointer, length, memory.data_size(&store), cap)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| ProbeError::Allocation)?;
    result.resize(length, 0);
    memory
        .read(&store, pointer, &mut result)
        .map_err(|_| ProbeError::OutputOutOfBounds)?;
    Ok(result)
}
fn require(value: bool) -> Result<(), ProbeError> {
    if value {
        Ok(())
    } else {
        Err(ProbeError::UnexpectedObservation)
    }
}
fn literal_and_range_checks(engine: &Engine) -> Result<usize, ProbeError> {
    let literal = module(engine, LITERAL)?;
    require(render(engine, &literal, EXPECTED.len())? == EXPECTED)?;
    require(render(engine, &literal, EXPECTED.len() - 1) == Err(ProbeError::OutputLimit))?;
    for (wat, expected) in [
        (
            r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) i32.const -1 i32.const 1))"#,
            ProbeError::NegativeOutput,
        ),
        (
            r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) i32.const 0 i32.const -1))"#,
            ProbeError::NegativeOutput,
        ),
        (
            r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) i32.const 65536 i32.const 1))"#,
            ProbeError::OutputOutOfBounds,
        ),
        (
            r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) i32.const 2147483647 i32.const 2147483647))"#,
            ProbeError::OutputOutOfBounds,
        ),
        (
            r#"(module (func (export "render") (result i32 i32) i32.const 0 i32.const 0))"#,
            ProbeError::ExportMismatch,
        ),
        (
            r#"(module (memory (export "memory") 1) (func (export "render") (result i32) i32.const 0))"#,
            ProbeError::ExportMismatch,
        ),
    ] {
        require(render(engine, &module(engine, wat)?, usize::MAX) == Err(expected))?;
    }
    let empty = module(
        engine,
        r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) i32.const 65536 i32.const 0))"#,
    )?;
    require(render(engine, &empty, 0)? == b"")?;
    Ok(9)
}
fn import_and_fuel_checks(engine: &Engine) -> Result<usize, ProbeError> {
    let imported = module(
        engine,
        r#"(module (import "fixture" "forbidden" (func)) (memory (export "memory") 1) (func (export "render") (result i32 i32) i32.const 0 i32.const 0))"#,
    )?;
    let mut store = store(engine, 1)?;
    let error = Linker::new(engine)
        .instantiate(&mut store, &imported)
        .err()
        .ok_or(ProbeError::UnexpectedObservation)?;
    let import = error
        .downcast_ref::<UnknownImportError>()
        .ok_or(ProbeError::UnexpectedObservation)?;
    require(import.module() == "fixture" && import.name() == "forbidden")?;
    require(classify(&error) == ProbeError::ImportRefused)?;
    for wat in [
        r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) (loop $spin br $spin) unreachable))"#,
        r#"(module (memory (export "memory") 1) (func $start (loop $spin br $spin)) (start $start) (func (export "render") (result i32 i32) i32.const 0 i32.const 0))"#,
    ] {
        require(render(engine, &module(engine, wat)?, 0) == Err(ProbeError::FuelExhausted))?;
    }
    let trap = module(
        engine,
        r#"(module (memory (export "memory") 1) (func (export "render") (result i32 i32) unreachable))"#,
    )?;
    require(render(engine, &trap, 0) == Err(ProbeError::GuestTrap))?;
    Ok(4)
}
fn isolation_checks(engine: &Engine) -> Result<usize, ProbeError> {
    let stateful = module(
        engine,
        r#"(module (memory (export "memory") 1)
      (global $count (mut i32) (i32.const 0))
      (func (export "render") (result i32 i32)
        global.get $count i32.const 1 i32.add global.set $count
        i32.const 0 global.get $count i32.store8 i32.const 0 i32.const 1))"#,
    )?;
    // Positive control: state persists when Store/instance are reused.
    let mut same_store = store(engine, 1)?;
    let same_instance = Linker::new(engine)
        .instantiate(&mut same_store, &stateful)
        .map_err(|_| ProbeError::Instantiate)?;
    let function = same_instance
        .get_typed_func::<(), (i32, i32)>(&mut same_store, "render")
        .map_err(|_| ProbeError::ExportMismatch)?;
    let memory = same_instance
        .get_memory(&mut same_store, "memory")
        .ok_or(ProbeError::ExportMismatch)?;
    for expected in [1, 2] {
        require(
            function
                .call(&mut same_store, ())
                .map_err(|error| classify(&error))?
                == (0, 1),
        )?;
        let mut byte = [0];
        memory
            .read(&same_store, 0, &mut byte)
            .map_err(|_| ProbeError::OutputOutOfBounds)?;
        require(byte == [expected])?;
    }
    for _ in 0..3 {
        require(render(engine, &stateful, 1)? == [1])?;
    }
    Ok(5)
}
fn allocation_checks(engine: &Engine) -> Result<usize, ProbeError> {
    // Pinned provider source's exact causes, not arbitrary errors. Every
    // fixture must compile successfully before its intended refusal is tested.
    for (wat, instances, reason) in [
        (
            "(module (memory 2))",
            1,
            "memory minimum size of 2 pages exceeds memory limits",
        ),
        (
            "(module (table 17 funcref))",
            1,
            "table minimum size of 17 elements exceeds table limits",
        ),
        (
            "(module (memory 1) (memory 1))",
            1,
            "resource limit exceeded: memory count too high at 2",
        ),
        (
            "(module (table 1 funcref) (table 1 funcref))",
            1,
            "resource limit exceeded: table count too high at 2",
        ),
        (
            "(module)",
            0,
            "resource limit exceeded: instance count too high at 1",
        ),
    ] {
        let module = module(engine, wat)?;
        let mut store = store(engine, instances)?;
        let error = Linker::new(engine)
            .instantiate(&mut store, &module)
            .err()
            .ok_or(ProbeError::UnexpectedObservation)?;
        require(error.chain().any(|cause| cause.to_string() == reason))?;
    }
    let growth = module(
        engine,
        r#"(module (memory (export "memory") 1) (table (export "table") 16 funcref)
      (func (export "grow_memory") (result i32) i32.const 1 memory.grow)
      (func (export "grow_table") (result i32) ref.null func i32.const 1 table.grow))"#,
    )?;
    let mut store = store(engine, 1)?;
    let instance = Linker::new(engine)
        .instantiate(&mut store, &growth)
        .map_err(|_| ProbeError::Instantiate)?;
    let memory_grow = instance
        .get_typed_func::<(), i32>(&mut store, "grow_memory")
        .map_err(|_| ProbeError::ExportMismatch)?;
    let table_grow = instance
        .get_typed_func::<(), i32>(&mut store, "grow_table")
        .map_err(|_| ProbeError::ExportMismatch)?;
    require(
        memory_grow
            .call(&mut store, ())
            .map_err(|_| ProbeError::GuestTrap)?
            == -1,
    )?;
    require(
        table_grow
            .call(&mut store, ())
            .map_err(|_| ProbeError::GuestTrap)?
            == -1,
    )?;
    require(
        instance
            .get_memory(&mut store, "memory")
            .ok_or(ProbeError::ExportMismatch)?
            .data_size(&store)
            == PAGE,
    )?;
    require(
        instance
            .get_table(&mut store, "table")
            .ok_or(ProbeError::ExportMismatch)?
            .size(&store)
            == 16,
    )?;
    Ok(9)
}
/// Fixed public fixtures only. The tuple is an experiment convention, not a
/// selected theme ABI or general arbitrary-Wasm execution API.
pub fn verify_fixed_fixtures() -> Result<Observations, ProbeError> {
    let engine = engine()?;
    let checks = literal_and_range_checks(&engine)?
        + import_and_fuel_checks(&engine)?
        + isolation_checks(&engine)?
        + allocation_checks(&engine)?;
    Ok(Observations {
        fixture_checks: checks,
        literal_output_bytes: EXPECTED.len(),
        fuel_per_store: FUEL,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_binary_and_range_export_contracts() {
        assert_eq!(literal_and_range_checks(&engine().unwrap()), Ok(9));
    }
    #[test]
    fn empty_linker_and_preinstantiation_fuel_cover_start_and_render() {
        assert_eq!(import_and_fuel_checks(&engine().unwrap()), Ok(4));
    }
    #[test]
    fn fresh_stores_do_not_share_mutable_guest_globals_or_memory() {
        assert_eq!(isolation_checks(&engine().unwrap()), Ok(5));
    }
    #[test]
    fn store_limits_refuse_initial_resources_counts_and_growth() {
        assert_eq!(allocation_checks(&engine().unwrap()), Ok(9));
    }
    #[test]
    fn range_checks_and_errors_have_no_guest_payload_or_provider_diagnostics() {
        assert_eq!(range(0, 0, 0, 0), Ok((0, 0)));
        assert_eq!(range(-1, 0, PAGE, 0), Err(ProbeError::NegativeOutput));
        assert_eq!(range(0, -1, PAGE, 0), Err(ProbeError::NegativeOutput));
        assert_eq!(range(1, 1, 1, 0), Err(ProbeError::OutputOutOfBounds));
        assert_eq!(range(0, 1, 1, 0), Err(ProbeError::OutputLimit));
        for error in [
            ProbeError::OutputLimit,
            ProbeError::FuelExhausted,
            ProbeError::Instantiate,
        ] {
            assert!(!format!("{error:?}: {error}").contains("PRIVATE_GUEST"));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}

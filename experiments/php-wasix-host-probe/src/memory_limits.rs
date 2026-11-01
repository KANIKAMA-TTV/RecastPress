//! WAT tests for the same safe policy used by the fixed PHP diagnostic.
//! Limits one imported shared linear memory, not RSS or compiler allocations.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use wasmer::{Memory, MemoryError, MemoryType, Module, Pages};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::runtime::task_manager::{SpawnType, TaskWasmRecycleProperties};
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking, WasiThreadError,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::memory_policy::{MemoryPolicyManager, apply_policy};
use super::{collect_pipe, make_engine};

// No memory export: exercise the same imported-only shape as the PHP guest.
// Five u32 observations are written into memory and read in the recycle hook.
const GROW_WAT: &str = r#"
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (import "env" "memory" (memory 403 65536 shared))
  (func (export "_start")
    i32.const 0
    memory.size
    i32.store
    i32.const 4
    i32.const 621
    memory.grow
    i32.store
    i32.const 8
    memory.size
    i32.store
    i32.const 12
    i32.const 1
    memory.grow
    i32.store
    i32.const 16
    memory.size
    i32.store
    i32.const 0
    call $exit))
"#;

#[derive(Debug)]
struct Observation {
    memory_type: MemoryType,
    pages: Pages,
    words: [u32; 5],
}

type Observations = Arc<Mutex<Vec<std::result::Result<Observation, String>>>>;

fn observe(props: &TaskWasmRecycleProperties) -> Result<Observation> {
    let mut wire = [0; 20];
    props.memory.view(&props.store).read(0, &mut wire)?;
    let words = std::array::from_fn(|index| {
        u32::from_le_bytes(
            wire[index * 4..index * 4 + 4]
                .try_into()
                .expect("four-byte word"),
        )
    });
    Ok(Observation {
        memory_type: props.memory.ty(&props.store),
        pages: props.memory.size(&props.store),
        words,
    })
}

fn run_fixture(cap: Option<Pages>) -> Result<Observation> {
    let data = HashedModuleData::new(wat::parse_str(GROW_WAT)?);
    let engine = make_engine();
    let module = Module::new(&engine, &data.wasm()[..])?;
    let observations: Observations = Arc::new(Mutex::new(Vec::new()));
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let guard = tokio_runtime.enter();
    let (stdin_sender, stdin_reader) = Pipe::channel();
    drop(stdin_sender);
    let (stdout_sender, stdout_reader) = Pipe::channel();
    let (stderr_sender, stderr_reader) = Pipe::channel();
    let outcome = (|| -> Result<()> {
        let captured = observations.clone();
        let manager = MemoryPolicyManager::new(
            TokioTaskManager::new(tokio_runtime.handle().clone()),
            cap,
            move |props| {
                let observed = observe(props).map_err(|error| format!("{error:#}"));
                captured
                    .lock()
                    .expect("observations mutex poisoned")
                    .push(observed);
            },
        );
        let mut runtime = PluggableRuntime::new(Arc::new(manager));
        runtime.set_engine(engine);
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let mut runner = WasiRunner::new();
        runner
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(false)
            .with_stdin(Box::new(stdin_reader))
            .with_stdout(Box::new(stdout_sender))
            .with_stderr(Box::new(stderr_sender));
        runner.capabilities_mut().threading.max_threads = Some(1);
        let result = runner.run_wasm(
            RuntimeOrEngine::Runtime(Arc::new(runtime)),
            "grow-test",
            module,
            *data.hash(),
        );
        drop(runner);
        result.context("run imported-memory fixture through real WasiRunner")
    })();
    drop(guard);
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    outcome?;
    ensure!(stdout?.is_empty(), "fixture produced unexpected stdout");
    ensure!(stderr?.is_empty(), "fixture produced unexpected stderr");
    let mut observations = observations.lock().expect("observations mutex poisoned");
    ensure!(
        observations.len() == 1,
        "expected one completed recycle observation"
    );
    observations
        .pop()
        .expect("one observation")
        .map_err(anyhow::Error::msg)
}

#[test]
fn runner_clamps_imported_shared_memory_and_rejects_growth_past_1024_pages() -> Result<()> {
    let observed = run_fixture(Some(Pages(1024)))?;
    assert_eq!(observed.words, [403, 403, 1024, u32::MAX, 1024]);
    assert_eq!(observed.pages, Pages(1024));
    assert_eq!(observed.memory_type.maximum, Some(Pages(1024)));
    assert!(observed.memory_type.shared);
    Ok(())
}

#[test]
fn runner_without_cap_allows_the_same_extra_page() -> Result<()> {
    let observed = run_fixture(None)?;
    assert_eq!(observed.words, [403, 403, 1024, 1024, 1025]);
    assert_eq!(observed.pages, Pages(1025));
    assert_eq!(observed.memory_type.maximum, Some(Pages(65536)));
    assert!(observed.memory_type.shared);
    Ok(())
}

#[test]
fn policy_rejects_initial_excess_and_existing_or_unknown_memory_paths() -> Result<()> {
    let mut initial = SpawnType::CreateMemoryOfType(MemoryType::new(403, Some(65536), true));
    assert!(matches!(
        apply_policy(&mut initial, Some(Pages(128))),
        Err(WasiThreadError::MemoryCreateFailed(
            MemoryError::MinimumMemoryTooLarge { .. }
        ))
    ));
    assert!(matches!(
        apply_policy(&mut SpawnType::CreateMemory, Some(Pages(1024))),
        Err(WasiThreadError::Unsupported)
    ));
    let mut store = wasmer::Store::new(make_engine());
    let memory = Memory::new(&mut store, MemoryType::new(1, Some(2), true))?;
    let shared = memory
        .as_shared(&store)
        .context("test backend must support shared memory")?;
    let mut attached = SpawnType::AttachMemory(shared);
    assert!(matches!(
        apply_policy(&mut attached, Some(Pages(1024))),
        Err(WasiThreadError::Unsupported)
    ));
    Ok(())
}

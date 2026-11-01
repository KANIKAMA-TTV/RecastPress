//! Test-only memory-policy experiment; not installed in the PHP runtime.
//! Limits one imported shared linear memory, not RSS or compiler allocations.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use wasmer::{Memory, MemoryError, MemoryType, Module, Pages, StoreMut};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::runtime::task_manager::{SpawnType, TaskWasm, TaskWasmRecycleProperties};
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking, VirtualTaskManager, WasiThreadError,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::{collect_pipe, make_engine};

type SharedFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type SharedTask = Box<dyn FnOnce() -> SharedFuture + Send + 'static>;

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

#[derive(Debug)]
struct MemoryPolicyManager {
    inner: TokioTaskManager,
    cap: Option<Pages>,
    observations: Observations,
}

/// This closed experiment accepts only creation of one imported shared
/// memory. Existing AttachMemory, internal-memory and linker-group routes
/// are rejected; it is not a general WASIX memory or process supervisor.
fn apply_policy(
    spawn: &mut SpawnType,
    cap: Option<Pages>,
) -> std::result::Result<(), WasiThreadError> {
    let SpawnType::CreateMemoryOfType(ty) = spawn else {
        return Err(WasiThreadError::Unsupported);
    };
    if !ty.shared {
        return Err(WasiThreadError::MemoryCreateFailed(
            MemoryError::MemoryNotShared,
        ));
    }
    if let Some(cap) = cap {
        if ty.minimum > cap {
            return Err(WasiThreadError::MemoryCreateFailed(
                MemoryError::MinimumMemoryTooLarge {
                    min_requested: ty.minimum,
                    max_allowed: cap,
                },
            ));
        }
        ty.maximum = Some(ty.maximum.map_or(cap, |original| original.min(cap)));
    }
    Ok(())
}

impl VirtualTaskManager for MemoryPolicyManager {
    fn build_memory(
        &self,
        store: &mut StoreMut,
        mut spawn: SpawnType,
    ) -> std::result::Result<Option<Memory>, WasiThreadError> {
        apply_policy(&mut spawn, self.cap)?;
        self.inner.build_memory(store, spawn)
    }

    fn sleep_now(
        &self,
        time: Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>> {
        self.inner.sleep_now(time)
    }

    fn task_shared(&self, task: SharedTask) -> std::result::Result<(), WasiThreadError> {
        self.inner.task_shared(task)
    }

    fn task_wasm(&self, mut task: TaskWasm) -> std::result::Result<(), WasiThreadError> {
        let info = task.module.info();
        if info.memories.len() != 1 || info.num_imported_memories != 1 {
            return Err(WasiThreadError::Unsupported);
        }
        let imported = task
            .module
            .imports()
            .memories()
            .next()
            .ok_or(WasiThreadError::Unsupported)?;
        if imported.module() != "env" || imported.name() != "memory" {
            return Err(WasiThreadError::Unsupported);
        }
        // TokioTaskManager does not call VirtualTaskManager::build_memory
        // here: rewrite the SpawnType that its new_with_store path consumes.
        apply_policy(&mut task.spawn_type, self.cap)?;
        let previous_recycle = task.callbacks.recycle.take();
        let observations = self.observations.clone();
        task.callbacks.recycle = Some(Box::new(move |props| {
            let observation = observe(&props).map_err(|error| format!("{error:#}"));
            observations
                .lock()
                .expect("observations mutex poisoned")
                .push(observation);
            if let Some(previous) = previous_recycle {
                previous(props);
            }
        }));
        self.inner.task_wasm(task)
    }

    fn task_dedicated(
        &self,
        task: Box<dyn FnOnce() + Send + 'static>,
    ) -> std::result::Result<(), WasiThreadError> {
        self.inner.task_dedicated(task)
    }

    fn thread_parallelism(&self) -> std::result::Result<usize, WasiThreadError> {
        self.inner.thread_parallelism()
    }
}

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
        let manager = MemoryPolicyManager {
            inner: TokioTaskManager::new(tokio_runtime.handle().clone()),
            cap,
            observations: observations.clone(),
        };
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

//! Shared safe policy for the fixed WAT and PHP diagnostics only.
//! One imported shared memory is bounded; RSS and aggregate use are not.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use wasmer::{Memory, MemoryError, Pages, StoreMut};
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::runtime::task_manager::{SpawnType, TaskWasm, TaskWasmRecycleProperties};
use wasmer_wasix::{VirtualTaskManager, WasiThreadError};

type SharedFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type SharedTask = Box<dyn FnOnce() -> SharedFuture + Send + 'static>;
type Observer = Arc<dyn Fn(&TaskWasmRecycleProperties) + Send + Sync>;

#[derive(Clone, Debug)]
pub(super) struct MemorySnapshot {
    pub maximum: Option<Pages>,
    pub pages: Pages,
    pub shared: bool,
}

/// Metadata only: never read arbitrary bytes from a PHP guest's memory.
pub(super) fn snapshot(props: &TaskWasmRecycleProperties) -> MemorySnapshot {
    let memory_type = props.memory.ty(&props.store);
    MemorySnapshot {
        maximum: memory_type.maximum,
        pages: props.memory.size(&props.store),
        shared: memory_type.shared,
    }
}

pub(super) struct MemoryPolicyManager {
    inner: TokioTaskManager,
    cap: Option<Pages>,
    observer: Observer,
}

impl fmt::Debug for MemoryPolicyManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryPolicyManager")
            .field("cap", &self.cap)
            .finish_non_exhaustive()
    }
}

impl MemoryPolicyManager {
    pub fn new(
        inner: TokioTaskManager,
        cap: Option<Pages>,
        observer: impl Fn(&TaskWasmRecycleProperties) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner,
            cap,
            observer: Arc::new(observer),
        }
    }
}

/// Existing AttachMemory, internal-memory and linker-group routes are
/// rejected. This is not a general WASIX process or memory supervisor.
pub(super) fn apply_policy(
    spawn: &mut SpawnType,
    cap: Option<Pages>,
) -> Result<(), WasiThreadError> {
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
    ) -> Result<Option<Memory>, WasiThreadError> {
        apply_policy(&mut spawn, self.cap)?;
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
        // Tokio's task_wasm does not dispatch through build_memory. Rewrite
        // the type passed to its new_with_store -> Memory::new path instead.
        apply_policy(&mut task.spawn_type, self.cap)?;
        let previous_recycle = task.callbacks.recycle.take();
        let observer = self.observer.clone();
        task.callbacks.recycle = Some(Box::new(move |props| {
            observer(&props);
            if let Some(previous) = previous_recycle {
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

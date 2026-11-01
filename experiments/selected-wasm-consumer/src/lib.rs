//! Fixed host-selected article/navigation Component experiment only.
//! Guest execution is outside publication locks. Final delivery moves only
//! completed copied HTML; it is not socket delivery, revocation, SDK or policy.
#![forbid(unsafe_code)]
use recastpress_core::publication::{
    self, PreparedSite, PublicPage, PublicReader, PublicSelection, ReadError, SelectionLimits,
};
use std::{
    borrow::Cow,
    fmt,
    str::Utf8Error,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use wasmtime::component::{Component, Instance, Linker, WasmStr};
use wasmtime::{Config, Engine, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder, Trap};

const DEADLINE: Duration = Duration::from_secs(5);
const TARGETS: [&str; 2] = ["/article?b=2&a=1", "/navigation"];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Compile,
    Profile,
    Selection,
    InputLimit,
    InvalidUtf8,
    OutputLimit,
    Allocation,
    MemoryLimit,
    Guest,
    Fuel,
    PostFuel,
    PostTrap,
    Checkpoint,
    WorkerUnconfirmed,
    Stale,
}
impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fixed selected Wasm consumer failed")
    }
}
impl std::error::Error for ProbeError {}
#[derive(Debug)]
struct CheckpointFailure;
impl fmt::Display for CheckpointFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fixed checkpoint failed")
    }
}
impl std::error::Error for CheckpointFailure {}

// Every caller explicitly supplies logical limits. These are not global RAM,
// CPU/time/concurrency defaults. The fixed experiment never accepts public input.
#[derive(Clone, Copy)]
struct Limits {
    selection: SelectionLimits,
    body: usize,
    input: usize,
    output: usize,
    fuel: u64,
    memory: usize,
}
fn limits(input: usize, output: usize, fuel: u64) -> Limits {
    Limits {
        selection: SelectionLimits::new(2, TARGETS.iter().map(|s| s.len()).sum()),
        body: 4096,
        input,
        output,
        fuel,
        memory: 65536,
    }
}
enum Checkpoint {
    None,
    #[cfg_attr(not(test), allow(dead_code))]
    Controlled {
        entered: mpsc::Sender<()>,
        ack: mpsc::Receiver<()>,
    },
}
struct Data {
    limits: StoreLimits,
    checkpoint: Checkpoint,
    calls: usize,
}
impl Data {
    fn checkpoint(&mut self) -> Result<(), wasmtime::Error> {
        self.calls += 1;
        match &self.checkpoint {
            Checkpoint::None => Ok(()),
            Checkpoint::Controlled { entered, ack } => {
                entered
                    .send(())
                    .map_err(|_| wasmtime::Error::new(CheckpointFailure))?;
                ack.recv_timeout(DEADLINE)
                    .map_err(|_| wasmtime::Error::new(CheckpointFailure))?;
                Ok(())
            }
        }
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
enum Mode {
    Normal,
    StartLoop,
    ReallocLoop,
    RenderLoop,
    PostLoop,
    InvalidUtf8,
    InvalidUtf8PostLoop,
}
fn source(mode: Mode) -> String {
    let start = if matches!(mode, Mode::StartLoop) {
        "(func $start (loop $spin br $spin)) (start $start)"
    } else {
        ""
    };
    let realloc = if matches!(mode, Mode::ReallocLoop) {
        "(loop $spin br $spin) unreachable"
    } else {
        "local.get 0 i32.eqz local.get 1 i32.eqz i32.and local.get 2 i32.const 1 i32.eq i32.and if else unreachable end
         global.get $arena local.get 3 i32.add i32.const 5120 i32.gt_u if unreachable end
         global.get $allocations i32.const 1 i32.add global.set $allocations
         global.get $arena global.get $arena local.get 3 i32.add global.set $arena"
    };
    let render = if matches!(mode, Mode::RenderLoop) {
        "(loop $spin br $spin) unreachable"
    } else {
        "global.get $counter i32.const 1 i32.add global.set $counter
         local.get 0 global.set $ap local.get 1 global.set $al local.get 2 global.set $np local.get 3 global.set $nl
         i32.const 8192 i32.const 32 i32.const 9 memory.copy
         i32.const 8201 local.get 0 local.get 1 memory.copy
         i32.const 8201 local.get 1 i32.add i32.const 64 i32.const 15 memory.copy
         i32.const 8216 local.get 1 i32.add local.get 2 local.get 3 memory.copy
         i32.const 8216 local.get 1 i32.add local.get 3 i32.add i32.const 96 i32.const 6 memory.copy
         i32.const 8 i32.const 8192 i32.store i32.const 12 local.get 1 local.get 3 i32.add i32.const 30 i32.add i32.store
         call $checkpoint INVALID i32.const 8"
    };
    let invalid = if matches!(mode, Mode::InvalidUtf8 | Mode::InvalidUtf8PostLoop) {
        "i32.const 8192 i32.const 255 i32.store8"
    } else {
        ""
    };
    let post = if matches!(mode, Mode::PostLoop | Mode::InvalidUtf8PostLoop) {
        "(loop $spin br $spin)"
    } else {
        "local.get 0 i32.const 8 i32.ne if unreachable end
         i32.const 8192 i32.const 88 i32.const 12 i32.load memory.fill
         i32.const 1024 global.set $arena global.get $postcalls i32.const 1 i32.add global.set $postcalls"
    };
    format!(
        r#"(component
      (import "fixture-checkpoint" (func $checkpoint))
      (core func $lowered (canon lower (func $checkpoint)))
      (core instance $host (export "checkpoint" (func $lowered)))
      (core module $m
        (import "host" "checkpoint" (func $checkpoint))
        (memory (export "memory") 1)
        (data (i32.const 32) "<article>") (data (i32.const 64) "</article><nav>") (data (i32.const 96) "</nav>")
        (global $arena (mut i32) (i32.const 1024)) (global $counter (mut i32) (i32.const 0))
        (global $allocations (mut i32) (i32.const 0)) (global $postcalls (mut i32) (i32.const 0))
        (global $ap (mut i32) (i32.const 0)) (global $al (mut i32) (i32.const 0))
        (global $np (mut i32) (i32.const 0)) (global $nl (mut i32) (i32.const 0))
        {start}
        (func (export "realloc") (param i32 i32 i32 i32) (result i32) {realloc})
        (func (export "render") (param i32 i32 i32 i32) (result i32) {render})
        (func (export "post") (param i32) {post})
        (func (export "peek") (result i32) i32.const 8)
        (func (export "counter") (result i32) global.get $counter)
        (func (export "allocations") (result i32) global.get $allocations)
        (func (export "postcalls") (result i32) global.get $postcalls)
        (func (export "nonoverlap") (result i32)
          global.get $ap global.get $al i32.add global.get $np i32.le_u
          global.get $np global.get $nl i32.add i32.const 5120 i32.le_u i32.and
          global.get $al i32.eqz global.get $nl i32.eqz i32.or i32.or))
      (core instance $i (instantiate $m (with "host" (instance $host))))
      (func (export "render") (param "article" string) (param "navigation" string) (result string)
        (canon lift (core func $i "render") (memory (core memory $i "memory"))
          (realloc (core func $i "realloc")) (post-return (core func $i "post"))))
      (func (export "peek") (result string) (canon lift (core func $i "peek") (memory (core memory $i "memory"))))
      (func (export "counter") (result u32) (canon lift (core func $i "counter")))
      (func (export "allocations") (result u32) (canon lift (core func $i "allocations")))
      (func (export "postcalls") (result u32) (canon lift (core func $i "postcalls")))
      (func (export "nonoverlap") (result u32) (canon lift (core func $i "nonoverlap"))))"#,
        render = render.replace("INVALID", invalid)
    )
}
fn classify(error: &wasmtime::Error) -> ProbeError {
    if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
        ProbeError::Fuel
    } else if error.downcast_ref::<CheckpointFailure>().is_some() {
        ProbeError::Checkpoint
    } else if error
        .chain()
        .any(|cause| cause.to_string() == "memory minimum size of 1 pages exceeds memory limits")
    {
        ProbeError::MemoryLimit
    } else {
        ProbeError::Guest
    }
}
struct Runtime {
    engine: Engine,
    component: Component,
    stores: AtomicU64,
}
impl Runtime {
    fn compile(mode: Mode) -> Result<Self, ProbeError> {
        let mut config = Config::new();
        config.consume_fuel(true).wasm_component_model(true);
        let engine = Engine::new(&config).map_err(|_| ProbeError::Compile)?;
        let component = Component::new(&engine, source(mode)).map_err(|_| ProbeError::Compile)?;
        let resources = component.resources_required().ok_or(ProbeError::Profile)?;
        if resources.num_memories != 1
            || resources.num_tables != 0
            || resources.max_initial_memory_size != Some(1)
        {
            return Err(ProbeError::Profile);
        }
        Ok(Self {
            engine,
            component,
            stores: AtomicU64::new(0),
        })
    }
    fn session(
        &self,
        limits: Limits,
        checkpoint: Checkpoint,
    ) -> Result<(Store<Data>, Instance), ProbeError> {
        let resource_limits = StoreLimitsBuilder::new()
            .instances(4)
            .memories(1)
            .tables(1)
            .memory_size(limits.memory)
            .table_elements(16)
            .build();
        let mut store = Store::new(
            &self.engine,
            Data {
                limits: resource_limits,
                checkpoint,
                calls: 0,
            },
        );
        store.limiter(|data| &mut data.limits);
        store
            .set_fuel(limits.fuel)
            .map_err(|_| ProbeError::Profile)?;
        let mut linker = Linker::new(&self.engine);
        linker
            .root()
            .func_wrap(
                "fixture-checkpoint",
                |mut context: StoreContextMut<'_, Data>, (): ()| context.data_mut().checkpoint(),
            )
            .map_err(|_| ProbeError::Profile)?;
        self.stores.fetch_add(1, Ordering::SeqCst);
        let instance = linker
            .instantiate(&mut store, &self.component)
            .map_err(|error| classify(&error))?;
        Ok((store, instance))
    }
    fn fresh(
        &self,
        article: &str,
        navigation: &str,
        limits: Limits,
        checkpoint: Checkpoint,
    ) -> Result<Rendered, ProbeError> {
        input_check(article.as_bytes(), navigation.as_bytes(), limits)?;
        let (mut store, instance) = self.session(limits, checkpoint)?;
        let rendered = render(
            &mut store,
            &instance,
            article,
            navigation,
            limits.output,
            false,
        )?;
        if rendered.counter != 1 || store.data().calls != 1 {
            return Err(ProbeError::Profile);
        }
        Ok(rendered)
    }
}
fn input_check(article: &[u8], navigation: &[u8], limits: Limits) -> Result<(), ProbeError> {
    if article.len() > limits.body
        || navigation.len() > limits.body
        || article
            .len()
            .checked_add(navigation.len())
            .filter(|&sum| sum <= limits.input && sum <= 4096)
            .is_none()
    {
        return Err(ProbeError::InputLimit);
    }
    std::str::from_utf8(article).map_err(|_| ProbeError::InvalidUtf8)?;
    std::str::from_utf8(navigation).map_err(|_| ProbeError::InvalidUtf8)?;
    Ok(())
}
#[derive(Debug)]
struct Rendered {
    bytes: Vec<u8>,
    counter: u32,
    allocations: u32,
}
fn scalar(store: &mut Store<Data>, instance: &Instance, name: &str) -> Result<u32, ProbeError> {
    let function = instance
        .get_typed_func::<(), (u32,)>(&mut *store, name)
        .map_err(|_| ProbeError::Profile)?;
    let (value,) = function
        .call(&mut *store, ())
        .map_err(|error| classify(&error))?;
    function
        .post_return(&mut *store)
        .map_err(|_| ProbeError::PostTrap)?;
    Ok(value)
}
fn render(
    store: &mut Store<Data>,
    instance: &Instance,
    article: &str,
    navigation: &str,
    cap: usize,
    fail_copy: bool,
) -> Result<Rendered, ProbeError> {
    let function = instance
        .get_typed_func::<(&str, &str), (WasmStr,)>(&mut *store, "render")
        .map_err(|_| ProbeError::Profile)?;
    let (value,) = function
        .call(&mut *store, (article, navigation))
        .map_err(|error| classify(&error))?;
    let tentative = (|| {
        let text = value.to_str(&*store).map_err(|error| {
            if error.downcast_ref::<Utf8Error>().is_some() {
                ProbeError::InvalidUtf8
            } else {
                ProbeError::Profile
            }
        })?;
        if !matches!(text, Cow::Borrowed(_)) {
            return Err(ProbeError::Profile);
        }
        if text.len() > cap {
            return Err(ProbeError::OutputLimit);
        }
        if fail_copy {
            return Err(ProbeError::Allocation);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(text.len())
            .map_err(|_| ProbeError::Allocation)?;
        bytes.extend_from_slice(text.as_bytes());
        Ok(bytes)
    })();
    function.post_return(&mut *store).map_err(|error| {
        if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
            ProbeError::PostFuel
        } else {
            ProbeError::PostTrap
        }
    })?;
    let peek = instance
        .get_typed_func::<(), (WasmStr,)>(&mut *store, "peek")
        .map_err(|_| ProbeError::Profile)?;
    let (value,) = peek
        .call(&mut *store, ())
        .map_err(|error| classify(&error))?;
    let mutation = value
        .to_str(&*store)
        .map(|text| {
            matches!(text, Cow::Borrowed(_))
                && text.len() == article.len() + navigation.len() + 30
                && text.bytes().all(|byte| byte == b'X')
        })
        .map_err(|_| ProbeError::Profile);
    peek.post_return(&mut *store)
        .map_err(|_| ProbeError::PostTrap)?;
    if !mutation? || scalar(store, instance, "nonoverlap")? != 1 {
        return Err(ProbeError::Profile);
    }
    let counter = scalar(store, instance, "counter")?;
    let allocations = scalar(store, instance, "allocations")?;
    let bytes = tentative?;
    Ok(Rendered {
        bytes,
        counter,
        allocations,
    })
}
fn select(reader: &PublicReader, limits: Limits) -> Result<PublicSelection, ProbeError> {
    reader
        .read_selected(&TARGETS, limits.selection)
        .map_err(|_| ProbeError::Selection)
}
fn render_selected(
    runtime: &Runtime,
    selection: &PublicSelection,
    limits: Limits,
    checkpoint: Checkpoint,
) -> Result<Rendered, ProbeError> {
    let mut pages = selection.pages();
    let article = pages.next().ok_or(ProbeError::Profile)?.body();
    let navigation = pages.next().ok_or(ProbeError::Profile)?.body();
    if pages.next().is_some() {
        return Err(ProbeError::Profile);
    }
    input_check(article, navigation, limits)?;
    runtime.fresh(
        std::str::from_utf8(article).map_err(|_| ProbeError::InvalidUtf8)?,
        std::str::from_utf8(navigation).map_err(|_| ProbeError::InvalidUtf8)?,
        limits,
        checkpoint,
    )
}
fn deliver(
    reader: &PublicReader,
    selection: &PublicSelection,
    rendered: Rendered,
    calls: &AtomicU64,
) -> Result<Vec<u8>, ProbeError> {
    reader
        .deliver_selected(selection, |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            rendered.bytes
        })
        .map_err(|error| {
            if error == ReadError::Stale {
                ProbeError::Stale
            } else {
                ProbeError::Selection
            }
        })
}
fn page(body: &[u8]) -> PublicPage {
    PublicPage::new(200, "text/html; charset=utf-8".into(), body.to_vec())
        .expect("fixed public fixture")
}
fn site(label: &str) -> PreparedSite {
    PreparedSite::new([
        (
            TARGETS[0].into(),
            page(format!("{label} article").as_bytes()),
        ),
        (
            TARGETS[1].into(),
            page(format!("{label} navigation").as_bytes()),
        ),
        ("/other".into(), page(b"unselected public")),
    ])
    .expect("fixed public fixture")
}

/// Fixed no-argument executable observation, not an accepted runtime SDK.
pub fn observe_current() -> Result<usize, ProbeError> {
    let runtime = Runtime::compile(Mode::Normal)?;
    let caps = limits(4096, 8192, 10000);
    let warm = runtime.fresh("warm article", "warm navigation", caps, Checkpoint::None)?;
    if warm.counter != 1 {
        return Err(ProbeError::Profile);
    }
    let (publisher, reader) = publication::publication_channel();
    publisher
        .publish(
            &publisher.revision().map_err(|_| ProbeError::Selection)?,
            site("A"),
        )
        .map_err(|_| ProbeError::Selection)?;
    let selection = select(&reader, caps)?;
    let rendered = render_selected(&runtime, &selection, caps, Checkpoint::None)?;
    if rendered.allocations != 2 {
        return Err(ProbeError::Profile);
    }
    let calls = AtomicU64::new(0);
    let html = deliver(&reader, &selection, rendered, &calls)?;
    if html != b"<article>A article</article><nav>A navigation</nav>"
        || calls.load(Ordering::SeqCst) != 1
    {
        return Err(ProbeError::Profile);
    }
    Ok(html.len())
}

#[cfg(test)]
mod tests;

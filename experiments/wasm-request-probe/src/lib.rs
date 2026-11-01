//! Fixed loopback request-time Component experiment, not SDK/engine/ABI or
//! production HTTP/auth policy. No visitor string is lowered into the guest.
//! Component compilation/warmup precede bind. GET/HEAD use only preselected
//! constants and fresh Stores; no handler compilation, file reads, jobs, WASI,
//! host definitions, PHP or DB. HTTP/runtime/compiler/allocation scheduling,
//! whole RSS/capacity/concurrency and wall time are not bounded by guest fuel.
#![forbid(unsafe_code)]
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::Response,
};
use std::{
    borrow::Cow,
    fmt,
    io::Write,
    str::Utf8Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use wasmtime::component::{Component, Instance, Linker, WasmStr};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap};

const HTML_A: &str = "<!doctype html><html><body><h1>Fixed A</h1></body></html>\n";
const HTML_B: &str =
    "<!doctype html><html><body><h1>Fixed B</h1><p>Fresh Store.</p></body></html>\n";
const LIMIT: usize = 4096;
const FUEL: u64 = 10_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Engine,
    Compile,
    Profile,
    Instantiate,
    InputLimit,
    OutputLimit,
    Allocation,
    InvalidUtf8,
    ReallocBounds,
    FuelExhausted,
    Guest,
    PostFuel,
    PostTrap,
    Io,
}
impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProbeError {}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Normal,
    #[cfg(test)]
    BadRealloc,
    #[cfg(test)]
    ReallocLoop,
    #[cfg(test)]
    RenderLoop,
    #[cfg(test)]
    PostLoop,
    #[cfg(test)]
    InvalidUtf8,
    #[cfg(test)]
    InvalidUtf8PostLoop,
}
fn source(mode: Mode) -> String {
    let realloc = match mode {
        #[cfg(test)]
        Mode::BadRealloc => "i32.const 65536",
        #[cfg(test)]
        Mode::ReallocLoop => "(loop $spin br $spin) unreachable",
        _ => {
            "local.get 0 i32.eqz local.get 1 i32.eqz i32.and local.get 2 i32.const 1 i32.eq i32.and if else unreachable end local.get 3 i32.const 4096 i32.gt_u if unreachable end global.get $allocations i32.const 1 i32.add global.set $allocations i32.const 1024"
        }
    };
    let render = match mode {
        #[cfg(test)]
        Mode::RenderLoop => "(loop $spin br $spin) unreachable",
        _ => {
            "global.get $counter i32.const 1 i32.add global.set $counter i32.const 8 local.get 0 i32.store i32.const 12 local.get 1 i32.store i32.const 8"
        }
    };
    let invalid = match mode {
        #[cfg(test)]
        Mode::InvalidUtf8 | Mode::InvalidUtf8PostLoop => {
            "local.get 1 if local.get 0 i32.const 255 i32.store8 end"
        }
        _ => "",
    };
    let post = match mode {
        #[cfg(test)]
        Mode::PostLoop | Mode::InvalidUtf8PostLoop => "(loop $spin br $spin)",
        _ => {
            "local.get 0 i32.const 8 i32.ne if unreachable end i32.const 8 i32.load i32.const 88 i32.const 12 i32.load memory.fill"
        }
    };
    format!(
        r#"(component
      (core module $m
        (memory (export "memory") 1)
        (global $counter (mut i32) (i32.const 0))
        (global $allocations (mut i32) (i32.const 0))
        (func (export "realloc") (param i32 i32 i32 i32) (result i32) {realloc})
        (func (export "render") (param i32 i32) (result i32) {invalid} {render})
        (func (export "peek") (result i32) i32.const 8)
        (func (export "post") (param i32) {post})
        (func (export "counter") (result i32) global.get $counter)
        (func (export "allocations") (result i32) global.get $allocations))
      (core instance $i (instantiate $m))
      (func (export "render") (param "value" string) (result string)
        (canon lift (core func $i "render") (memory (core memory $i "memory"))
          (realloc (core func $i "realloc")) (post-return (core func $i "post"))))
      (func (export "peek") (result string) (canon lift (core func $i "peek") (memory (core memory $i "memory"))))
      (func (export "counter") (result u32) (canon lift (core func $i "counter")))
      (func (export "allocations") (result u32) (canon lift (core func $i "allocations"))))"#
    )
}
fn classify(error: &wasmtime::Error) -> ProbeError {
    if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
        ProbeError::FuelExhausted
    } else if error
        .chain()
        .any(|cause| cause.to_string() == "realloc return: beyond end of memory")
    {
        ProbeError::ReallocBounds
    } else {
        ProbeError::Guest
    }
}
struct Runtime {
    engine: Engine,
    component: Component,
    stores: AtomicU64,
    compilations: usize,
    warmups: usize,
}
struct Rendered {
    bytes: Vec<u8>,
    counter: u32,
    allocations: u32,
}
impl Runtime {
    fn compile(mode: Mode) -> Result<Self, ProbeError> {
        let mut configuration = Config::new();
        configuration.consume_fuel(true).wasm_component_model(true);
        let engine = Engine::new(&configuration).map_err(|_| ProbeError::Engine)?;
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
            compilations: 1,
            warmups: 0,
        })
    }
    fn startup() -> Result<Self, ProbeError> {
        let mut runtime = Self::compile(Mode::Normal)?;
        let warm = runtime.fresh(HTML_A, LIMIT)?;
        if warm.bytes != HTML_A.as_bytes() || warm.counter != 1 || warm.allocations != 1 {
            return Err(ProbeError::Profile);
        }
        runtime.warmups = 1;
        Ok(runtime)
    }
    fn session(&self) -> Result<(Store<StoreLimits>, Instance), ProbeError> {
        let limits = StoreLimitsBuilder::new()
            .instances(1)
            .memories(1)
            .tables(1)
            .memory_size(65_536)
            .table_elements(16)
            .build();
        self.stores.fetch_add(1, Ordering::SeqCst);
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|limits| limits);
        store.set_fuel(FUEL).map_err(|_| ProbeError::Engine)?;
        let instance = Linker::new(&self.engine)
            .instantiate(&mut store, &self.component)
            .map_err(|_| ProbeError::Instantiate)?;
        Ok((store, instance))
    }
    fn fresh(&self, input: &str, maximum: usize) -> Result<Rendered, ProbeError> {
        if input.len() > LIMIT {
            return Err(ProbeError::InputLimit);
        }
        let (mut store, instance) = self.session()?;
        echo(&mut store, &instance, input, maximum)
    }
}
fn scalar(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    name: &str,
) -> Result<u32, ProbeError> {
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
fn echo(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    input: &str,
    maximum: usize,
) -> Result<Rendered, ProbeError> {
    if input.len() > LIMIT {
        return Err(ProbeError::InputLimit);
    }
    let render = instance
        .get_typed_func::<(&str,), (WasmStr,)>(&mut *store, "render")
        .map_err(|_| ProbeError::Profile)?;
    let (value,) = render
        .call(&mut *store, (input,))
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
        if text.len() > maximum {
            return Err(ProbeError::OutputLimit);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(text.len())
            .map_err(|_| ProbeError::Allocation)?;
        output.extend_from_slice(text.as_bytes());
        Ok(output)
    })();
    // callOk always requires cleanup, including UTF8/cap/copy refusal.
    render.post_return(&mut *store).map_err(|error| {
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
    let changed = (|| {
        let text = value.to_str(&*store).map_err(|_| ProbeError::Profile)?;
        Ok::<_, ProbeError>(
            matches!(text, Cow::Borrowed(_))
                && text.len() == input.len()
                && text.bytes().all(|byte| byte == b'X'),
        )
    })();
    peek.post_return(&mut *store)
        .map_err(|_| ProbeError::PostTrap)?;
    if !changed? {
        return Err(ProbeError::Profile);
    }
    let counter = scalar(store, instance, "counter")?;
    let allocations = scalar(store, instance, "allocations")?;
    let bytes = tentative?;
    if bytes != input.as_bytes() {
        return Err(ProbeError::Profile);
    }
    Ok(Rendered {
        bytes,
        counter,
        allocations,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Plan {
    fixture: &'static str,
    input: &'static str,
    head: bool,
}
fn plan(
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    authority: &str,
) -> Result<Plan, StatusCode> {
    let mut hosts = headers.get_all(header::HOST).iter();
    if hosts.next().map(|value| value.as_bytes()) != Some(authority.as_bytes())
        || hosts.next().is_some()
    {
        return Err(StatusCode::MISDIRECTED_REQUEST);
    }
    if method != Method::GET && method != Method::HEAD {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    if uri.scheme().is_some() || uri.authority().is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if uri.query().is_some() {
        return Err(StatusCode::NOT_FOUND);
    }
    let (fixture, input) = match uri.path() {
        "/a" => ("a", HTML_A),
        "/b" => ("b", HTML_B),
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok(Plan {
        fixture,
        input,
        head: method == Method::HEAD,
    })
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Call {
    ordinal: u64,
    fixture: &'static str,
    store_counter: u32,
    input_bytes: usize,
    output_bytes: usize,
}
fn emit_call(call: Call) -> Result<(), ProbeError> {
    emit(
        "WASM_REQUEST_CALL=",
        serde_json::json!({"format_version":1,"ordinal":call.ordinal,"fixture":call.fixture,"store_counter":call.store_counter,"input_bytes":call.input_bytes,"output_bytes":call.output_bytes}),
    )
}
fn emit(prefix: &str, value: serde_json::Value) -> Result<(), ProbeError> {
    let mut wire = prefix.as_bytes().to_vec();
    serde_json::to_writer(&mut wire, &value).map_err(|_| ProbeError::Io)?;
    wire.push(b'\n');
    let mut output = std::io::stderr().lock();
    output
        .write_all(&wire)
        .and_then(|_| output.flush())
        .map_err(|_| ProbeError::Io)
}
struct App {
    runtime: Runtime,
    authority: String,
    ordinal: AtomicU64,
}
fn empty(status: StatusCode) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_LENGTH, "0")
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff");
    if status == StatusCode::METHOD_NOT_ALLOWED {
        response = response.header(header::ALLOW, "GET, HEAD");
    }
    response
        .body(Body::empty())
        .expect("fixed response headers")
}
impl App {
    fn respond(
        &self,
        headers: &HeaderMap,
        method: &Method,
        uri: &Uri,
        log: impl FnOnce(Call) -> Result<(), ProbeError>,
    ) -> Response {
        let selected = match plan(headers, method, uri, &self.authority) {
            Ok(value) => value,
            Err(status) => return empty(status),
        };
        let rendered = match self.runtime.fresh(selected.input, LIMIT) {
            Ok(value) => value,
            Err(_) => return empty(StatusCode::INTERNAL_SERVER_ERROR),
        };
        if rendered.counter != 1 || rendered.allocations != 1 {
            return empty(StatusCode::INTERNAL_SERVER_ERROR);
        }
        let ordinal = match self
            .ordinal
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            }) {
            Ok(value) => value + 1,
            Err(_) => return empty(StatusCode::INTERNAL_SERVER_ERROR),
        };
        let length = rendered.bytes.len();
        if log(Call {
            ordinal,
            fixture: selected.fixture,
            store_counter: rendered.counter,
            input_bytes: selected.input.len(),
            output_bytes: length,
        })
        .is_err()
        {
            return empty(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CONTENT_LENGTH, length.to_string())
            .header(header::CACHE_CONTROL, "no-store")
            .header("x-content-type-options", "nosniff")
            .body(if selected.head {
                Body::empty()
            } else {
                Body::from(rendered.bytes)
            })
            .expect("fixed response headers")
    }
}
async fn handler(State(app): State<Arc<App>>, request: Request) -> Response {
    app.respond(
        request.headers(),
        request.method(),
        request.uri(),
        emit_call,
    )
}
/// Compile/warm the only fixed component before any listener is bound. No
/// caller-controlled module, HTML, port, filesystem path or host binding.
/// The owned parent terminates/reaps this loopback experiment after observation.
pub async fn run_loopback() -> Result<(), ProbeError> {
    let runtime = Runtime::startup()?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| ProbeError::Io)?;
    let authority = listener
        .local_addr()
        .map_err(|_| ProbeError::Io)?
        .to_string();
    let ready = serde_json::json!({"format_version":1,"profile":"wasmtime_36_0_19_fixed_request_time_component","process_pid":std::process::id(),"authority":authority,"compilations":runtime.compilations,"warmups":runtime.warmups});
    let app = Arc::new(App {
        runtime,
        authority,
        ordinal: AtomicU64::new(0),
    });
    let router = Router::new().fallback(handler).with_state(app);
    emit("WASM_REQUEST_READY=", ready)?;
    axum::serve(listener, router)
        .await
        .map_err(|_| ProbeError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_utf8_lowering_empty_nul_unicode_and_exact_input_cap() {
        let runtime = Runtime::compile(Mode::Normal).unwrap();
        for input in ["", "\0", "日本語é🦀", "ASCII\0日本語"] {
            let saved = input.to_owned();
            let result = runtime.fresh(input, LIMIT).unwrap();
            assert_eq!(result.bytes, input.as_bytes());
            assert_eq!(result.counter, 1);
            assert_eq!(result.allocations, 1);
            assert_eq!(input, saved);
        }
        assert_eq!(
            runtime
                .fresh(&"x".repeat(LIMIT), LIMIT)
                .unwrap()
                .bytes
                .len(),
            LIMIT
        );
        let before = runtime.stores.load(Ordering::SeqCst);
        assert_eq!(
            runtime.fresh(&"x".repeat(LIMIT + 1), LIMIT).err(),
            Some(ProbeError::InputLimit)
        );
        assert_eq!(runtime.stores.load(Ordering::SeqCst), before);
    }
    #[test]
    fn same_store_positive_control_then_fresh_store_always_resets() {
        let runtime = Runtime::compile(Mode::Normal).unwrap();
        let (mut store, instance) = runtime.session().unwrap();
        for (input, counter) in [("first", 1), ("second", 2)] {
            let value = echo(&mut store, &instance, input, LIMIT).unwrap();
            assert_eq!(value.counter, counter);
            assert_eq!(value.allocations, counter);
            assert_eq!(value.bytes, input.as_bytes());
        }
        for input in [HTML_A, HTML_B, HTML_A] {
            assert_eq!(runtime.fresh(input, LIMIT).unwrap().counter, 1);
        }
    }
    #[test]
    fn lowering_bounds_and_three_guest_fuel_phases_have_precise_refusals() {
        for (mode, expected) in [
            (Mode::BadRealloc, ProbeError::ReallocBounds),
            (Mode::ReallocLoop, ProbeError::FuelExhausted),
            (Mode::RenderLoop, ProbeError::FuelExhausted),
            (Mode::PostLoop, ProbeError::PostFuel),
        ] {
            assert_eq!(
                Runtime::compile(mode)
                    .unwrap()
                    .fresh("fixture", LIMIT)
                    .err(),
                Some(expected)
            );
        }
    }
    #[test]
    fn host_refusal_still_cleans_and_post_fuel_overrides_tentative_errors() {
        let runtime = Runtime::compile(Mode::Normal).unwrap();
        let (mut store, instance) = runtime.session().unwrap();
        assert_eq!(
            echo(&mut store, &instance, "fixture", 0).err(),
            Some(ProbeError::OutputLimit)
        );
        assert_eq!(scalar(&mut store, &instance, "counter").unwrap(), 1);
        assert_eq!(
            echo(&mut store, &instance, "next", LIMIT).unwrap().counter,
            2
        );
        assert_eq!(
            Runtime::compile(Mode::InvalidUtf8)
                .unwrap()
                .fresh("fixture", LIMIT)
                .err(),
            Some(ProbeError::InvalidUtf8)
        );
        assert_eq!(
            Runtime::compile(Mode::InvalidUtf8PostLoop)
                .unwrap()
                .fresh("fixture", LIMIT)
                .err(),
            Some(ProbeError::PostFuel)
        );
        assert_eq!(
            Runtime::compile(Mode::PostLoop)
                .unwrap()
                .fresh("fixture", 0)
                .err(),
            Some(ProbeError::PostFuel)
        );
    }
    #[test]
    fn host_method_query_absolute_and_unknown_refusals_never_enter_wasm() {
        let app = App {
            runtime: Runtime::startup().unwrap(),
            authority: "127.0.0.1:1234".into(),
            ordinal: AtomicU64::new(0),
        };
        let before = app.runtime.stores.load(Ordering::SeqCst);
        let mut good = HeaderMap::new();
        good.insert(header::HOST, "127.0.0.1:1234".parse().unwrap());
        let mut duplicate = good.clone();
        duplicate.append(header::HOST, "127.0.0.1:1234".parse().unwrap());
        for (headers, method, uri, status) in [
            (HeaderMap::new(), Method::GET, "/a", 421),
            (duplicate, Method::GET, "/a", 421),
            (good.clone(), Method::POST, "/a", 405),
            (good.clone(), Method::GET, "/a?x=private", 404),
            (good.clone(), Method::GET, "/unknown", 404),
            (good, Method::GET, "http://127.0.0.1:1234/a", 400),
        ] {
            let response = app.respond(&headers, &method, &uri.parse().unwrap(), |_| {
                panic!("refused request emitted CALL")
            });
            assert_eq!(response.status().as_u16(), status);
            if status == 405 {
                assert_eq!(response.headers()[header::ALLOW], "GET, HEAD");
            }
        }
        assert_eq!(app.runtime.stores.load(Ordering::SeqCst), before);
        assert_eq!(app.ordinal.load(Ordering::SeqCst), 0);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn get_head_fixed_bodies_call_order_and_prelisten_warmup_are_observable() {
        assert_eq!((HTML_A.len(), HTML_B.len()), (58, 77));
        let app = App {
            runtime: Runtime::startup().unwrap(),
            authority: "127.0.0.1:1234".into(),
            ordinal: AtomicU64::new(0),
        };
        assert_eq!((app.runtime.compilations, app.runtime.warmups), (1, 1));
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:1234".parse().unwrap());
        let mut calls = Vec::new();
        for (method, path, body) in [
            (Method::GET, "/a", HTML_A),
            (Method::GET, "/b", HTML_B),
            (Method::GET, "/a", HTML_A),
            (Method::HEAD, "/a", HTML_A),
        ] {
            let response = app.respond(&headers, &method, &path.parse().unwrap(), |call| {
                calls.push(call);
                Ok(())
            });
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CONTENT_LENGTH],
                body.len().to_string()
            );
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()["x-content-type-options"], "nosniff");
            let bytes = axum::body::to_bytes(response.into_body(), LIMIT)
                .await
                .unwrap();
            assert_eq!(
                bytes.as_ref(),
                if method == Method::HEAD {
                    b""
                } else {
                    body.as_bytes()
                }
            );
        }
        assert_eq!(
            calls.iter().map(|value| value.ordinal).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert!(
            calls
                .iter()
                .all(|value| value.store_counter == 1 && value.input_bytes == value.output_bytes)
        );
        assert_eq!(app.runtime.stores.load(Ordering::SeqCst), 5);
    }
    #[test]
    fn closed_errors_and_call_metadata_contain_no_visitor_or_guest_content() {
        for error in [
            ProbeError::Guest,
            ProbeError::InputLimit,
            ProbeError::PostFuel,
        ] {
            assert!(std::error::Error::source(&error).is_none());
            assert!(!format!("{error:?}: {error}").contains("PRIVATE"));
        }
        let value = Call {
            ordinal: 1,
            fixture: "a",
            store_counter: 1,
            input_bytes: 58,
            output_bytes: 58,
        };
        assert!(!format!("{value:?}").contains(HTML_A));
    }
}

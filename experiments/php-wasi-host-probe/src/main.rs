use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::mpsc, thread, time::Duration};
use wasmtime::{
    Config, Engine, Linker, Module, ResourceLimiter, Store, StoreLimits, StoreLimitsBuilder, Trap,
};
use wasmtime_wasi::{
    DirPerms, FilePerms, I32Exit, WasiCtxBuilder,
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
    preview1::{self, WasiP1Ctx},
};

mod file_bridge;
use file_bridge::Bridge;

const FIXTURE_SHA256: &str = "4fd2e8c42ae529ba72f88a0f1e46de1fc69a4b4f01e01fedd65ca966b8ffe6fa";
const FIXTURE_BYTES: u64 = 6_259_973;
const FUEL: u64 = 200_000_000;
const MEMORY: usize = 256 * 1024 * 1024;
const OUTPUT: usize = file_bridge::OUTPUT_LIMIT;
const BODY: &[u8] = b"rp\0\xff\r\nbody";
const WORDPRESS_FIXTURE: &str = include_str!("../wordpress-fixture.json");

fn wordpress_directory() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.reference/wordpress-7.1.3/wp-includes")
}

fn verify_wordpress_sources() -> Result<()> {
    use std::io::Read;
    let fixture: serde_json::Value = serde_json::from_str(WORDPRESS_FIXTURE)?;
    ensure!(
        fixture["wordpress_version"] == "7.1.3",
        "wrong WordPress fixture version"
    );
    for name in [
        "plugin.php",
        "class-wp-hook.php",
        "class-wp-filter-sentinel.php",
    ] {
        let expected = &fixture["files"][name];
        let size = expected["bytes"]
            .as_u64()
            .context("missing WP source length")?;
        ensure!(size <= 1024 * 1024, "WP source is too large");
        let file = std::fs::File::open(wordpress_directory().join(name))
            .context("prepare the pinned WordPress reference first")?;
        ensure!(
            file.metadata()?.len() == size,
            "wrong WP source length: {name}"
        );
        let mut bytes = Vec::new();
        file.take(size + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == size,
            "WP source changed while reading"
        );
        ensure!(
            format!("{:x}", Sha256::digest(&bytes))
                == expected["sha256"]
                    .as_str()
                    .context("missing WP source hash")?,
            "wrong WP source hash: {name}"
        );
    }
    Ok(())
}

struct State {
    wasi: WasiP1Ctx,
    limits: StoreLimits,
    memory_denials: usize,
}
impl ResourceLimiter for State {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool> {
        if desired > MEMORY {
            self.memory_denials += 1;
        }
        self.limits.memory_growing(current, desired, maximum)
    }
    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool> {
        self.limits.table_growing(current, desired, maximum)
    }
    fn memory_grow_failed(&mut self, error: anyhow::Error) -> Result<()> {
        self.limits.memory_grow_failed(error)
    }
    fn table_grow_failed(&mut self, error: anyhow::Error) -> Result<()> {
        self.limits.table_grow_failed(error)
    }
    fn instances(&self) -> usize {
        self.limits.instances()
    }
    fn memories(&self) -> usize {
        self.limits.memories()
    }
    fn tables(&self) -> usize {
        self.limits.tables()
    }
}

// Cancel and join each finished request's deadline thread.
struct Deadline {
    cancel: mpsc::Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Deadline {
    fn new(engine: Engine, duration: Duration) -> Self {
        let (cancel, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            if matches!(
                receiver.recv_timeout(duration),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                engine.increment_epoch();
            }
        });
        Self {
            cancel,
            thread: Some(thread),
        }
    }
}
impl Drop for Deadline {
    fn drop(&mut self) {
        let _ = self.cancel.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Run {
    exit: Result<()>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    bridge: Bridge,
    fuel_left: u64,
    memory_denials: usize,
}
impl Run {
    fn completed_body(&self) -> Result<&[u8]> {
        // Never publish a partially successful guest result.
        self.exit
            .as_ref()
            .map_err(|error| anyhow::anyhow!("guest failed: {error:#}"))?;
        ensure!(self.stdout.len() < OUTPUT, "response output limit");
        ensure!(self.stderr.len() < 8192, "stderr output limit");
        self.bridge.validate()?;
        let split = self
            .stdout
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .map(|position| position + 4)
            .or_else(|| {
                self.stdout
                    .windows(2)
                    .position(|bytes| bytes == b"\n\n")
                    .map(|position| position + 2)
            })
            .context("missing complete CGI headers")?;
        Ok(&self.stdout[split..])
    }
}

fn run(
    engine: &Engine,
    module: &Module,
    mode: &str,
    nonce: u64,
    fuel: u64,
    timeout: Duration,
) -> Result<Run> {
    let stderr = MemoryOutputPipe::new(8192);
    let bridge = Bridge::new(nonce)?;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    ensure!(
        fixtures.parent().unwrap().join("outside.txt").is_file(),
        "outside sentinel is missing"
    );
    ensure!(
        !fixtures.join("blocked.txt").exists(),
        "write-denial sentinel already exists"
    );
    let mut builder = WasiCtxBuilder::new();
    builder
        .args(&[
            "php-cgi",
            "-n",
            "-d",
            "cgi.force_redirect=0",
            "-d",
            "expose_php=0",
            "-d",
            "display_errors=stderr",
            "-d",
            "html_errors=0",
            "-d",
            "memory_limit=-1",
            "-f",
            "/site/probe.php",
        ])
        .env("REQUEST_METHOD", "POST")
        .env("CONTENT_TYPE", "application/octet-stream")
        .env("CONTENT_LENGTH", BODY.len().to_string())
        .env("SCRIPT_FILENAME", "/site/probe.php")
        .env("SCRIPT_NAME", "/probe.php")
        .env("SERVER_PROTOCOL", "HTTP/1.1")
        .env("SERVER_NAME", "rp.invalid")
        .env("REDIRECT_STATUS", "200")
        .env("HTTP_COOKIE", format!("rp_fixture={nonce}"))
        .env("RP_PROBE_MODE", mode)
        .allow_tcp(false)
        .allow_udp(false)
        .allow_ip_name_lookup(false)
        .stdin(MemoryInputPipe::new(BODY))
        .stdout(bridge.output())
        .stderr(stderr.clone());
    builder.preopened_dir(&fixtures, "/site", DirPerms::READ, FilePerms::READ)?;
    builder.preopened_dir(
        wordpress_directory(),
        "/wp-core",
        DirPerms::READ,
        FilePerms::READ,
    )?;
    builder.preopened_dir(
        bridge.directory(),
        "/rp-p0-rpc",
        DirPerms::READ,
        FilePerms::READ,
    )?;
    let mut store = Store::new(
        engine,
        State {
            wasi: builder.build_p1(),
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY)
                .table_elements(100_000)
                .instances(1)
                .memories(1)
                .tables(1)
                .trap_on_grow_failure(true)
                .build(),
            memory_denials: 0,
        },
    );
    store.limiter(|state| state);
    store.set_fuel(fuel)?;
    store.set_epoch_deadline(1);
    let mut linker = Linker::new(engine);
    preview1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)?;
    let _deadline = Deadline::new(engine.clone(), timeout);
    let exit = (|| {
        let instance = linker.instantiate(&mut store, module)?;
        let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
        match start.call(&mut store, ()) {
            Err(error)
                if error
                    .downcast_ref::<I32Exit>()
                    .is_some_and(|exit| exit.0 == 0) =>
            {
                Ok(())
            }
            result => result,
        }
    })();
    let fuel_left = store.get_fuel()?;
    let memory_denials = store.data().memory_denials;
    ensure!(
        !fixtures.join("blocked.txt").exists(),
        "guest created a file in its read-only preopen"
    );
    drop(store); // Dispose PHP globals, streams, handles, and memory on every path.
    let stdout = bridge.captured();
    bridge.cleanup()?;
    Ok(Run {
        exit,
        stdout,
        stderr: stderr.contents().to_vec(),
        bridge,
        fuel_left,
        memory_denials,
    })
}

fn check_basic(engine: &Engine, module: &Module, nonce: u64) -> Result<()> {
    let run = run(engine, module, "basic", nonce, FUEL, Duration::from_secs(5))?;
    let body = run.completed_body().with_context(|| {
        format!(
            "basic CGI output={:?}; stderr={:?}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        )
    })?;
    let value: serde_json::Value = serde_json::from_slice(body).context("PHP response JSON")?;
    ensure!(
        value["version"] == "8.2.6",
        "wrong PHP interpreter version: {value}"
    );
    ensure!(value["include"] == "included-from-wasi", "include failed");
    ensure!(
        value["input_hex"] == "727000ff0d0a626f6479",
        "binary request input mismatch"
    );
    ensure!(
        value["cookie"] == nonce.to_string(),
        "request cookie mismatch"
    );
    ensure!(
        value["counter"] == 1 && value["shared_counter"] == 2,
        "request state mismatch"
    );
    ensure!(
        value["cache_before"].is_null() && value["cache_after"] == 7,
        "cache callback or request cleanup failed"
    );
    ensure!(
        value["db_nonce"] == nonce && value["db_rows"] == 1,
        "live Rust DB fixture reply mismatch"
    );
    ensure!(
        value["outside_denied"] == true
            && value["write_denied"] == true
            && value["env_absent"] == true,
        "capability boundary failed"
    );
    run.bridge.check_calls(4)?;
    ensure!(
        value["wp_hooks_initially_empty"] == true,
        "WP hooks leaked between requests"
    );
    ensure!(
        value["wp_filter"] == "post|plugin|theme",
        "WP filter ordering or arguments failed"
    );
    ensure!(
        value["wp_action_counter"] == 1 && value["wp_action_calls"] == 1,
        "WP action/shared object failed"
    );
    ensure!(
        value["wp_null_result"] == true,
        "WP null/sentinel semantics failed"
    );
    Ok(())
}

fn verify(trace: bool) -> Result<serde_json::Value> {
    ensure!(
        std::env::args_os().len() == if trace { 2 } else { 1 },
        "fixed-fixture probe accepts only --trace"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    verify_wordpress_sources()?;
    let artifact = root.join(".reference/php-wasi-8.2.6/php-cgi-8.2.6-slim.wasm");
    let file =
        std::fs::File::open(&artifact).context("prepare the pinned PHP WASI fixture first")?;
    ensure!(
        file.metadata()?.len() == FIXTURE_BYTES,
        "wrong fixture length"
    );
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(FIXTURE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == FIXTURE_BYTES,
        "fixture changed while reading"
    );
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == FIXTURE_SHA256,
        "wrong fixture hash"
    );
    let mut config = Config::new();
    config
        .consume_fuel(true)
        .epoch_interruption(true)
        .cranelift_opt_level(wasmtime::OptLevel::None);
    let engine = Engine::new(&config)?;
    let started = std::time::Instant::now();
    if trace {
        eprintln!("P0: compiling the pinned PHP module");
    }
    let module = Module::new(&engine, &bytes)?;
    if trace {
        eprintln!("P0: module compiled in {:?}", started.elapsed());
    }
    for import in module.imports() {
        ensure!(
            import.module() == "wasi_snapshot_preview1",
            "non-WASI import: {}::{}",
            import.module(),
            import.name()
        );
    }
    if trace {
        eprintln!("P0: first basic request");
    }
    check_basic(&engine, &module, 4117)?;
    if trace {
        eprintln!("P0: second basic request");
    }
    check_basic(&engine, &module, 8229)?;
    let mut checks = vec![
        "real_php_wasi",
        "binary_input",
        "cgi_output",
        "include_read",
        "shared_request_state",
        "live_rust_cache_callback",
        "live_rust_db_fixture_callback",
        "fresh_request_state",
        "read_only_preopen",
        "outside_file_denied",
        "no_inherited_environment",
        "wp_filter_chain",
        "wp_actions_shared_object",
        "wp_null_sentinel",
        "wp_request_hook_cleanup",
    ];
    for (mode, fuel, timeout, trap) in [
        ("fuel", FUEL, Duration::from_secs(5), Some(Trap::OutOfFuel)),
        (
            "epoch",
            u64::MAX,
            Duration::from_millis(100),
            Some(Trap::Interrupt),
        ),
        ("fatal", FUEL, Duration::from_secs(5), None),
        ("output", FUEL, Duration::from_secs(5), None),
        ("memory", FUEL, Duration::from_secs(5), None),
        ("bad_rpc", FUEL, Duration::from_secs(5), None),
    ] {
        if trace {
            eprintln!("P0: failure/recovery fixture {mode}");
        }
        let result = run(&engine, &module, mode, 123, fuel, timeout)?;
        ensure!(
            result.completed_body().is_err(),
            "failed request became publishable: {mode}"
        );
        match mode {
            "fatal" => {
                ensure!(result.exit.is_err(), "PHP fatal did not fail");
                ensure!(
                    result
                        .stdout
                        .windows(b"partial response".len())
                        .any(|bytes| bytes == b"partial response"),
                    "fatal fixture did not produce partial output"
                );
                result.bridge.check_calls(1)?;
            }
            "output" => ensure!(
                result.stdout.len() == OUTPUT,
                "output fixture did not reach its limit"
            ),
            "memory" => ensure!(
                result.memory_denials > 0,
                "memory fixture did not reach the Wasm limit"
            ),
            "bad_rpc" => {
                ensure!(result.exit.is_err(), "unknown RPC did not trap");
                result.bridge.check_calls(1)?;
            }
            _ => {}
        }
        if let Some(trap) = trap {
            let error = result.exit.as_ref().err().context("expected Wasm trap")?;
            ensure!(
                error.downcast_ref::<Trap>() == Some(&trap),
                "wrong {mode} trap: {error:#}; fuel left={}",
                result.fuel_left
            );
        }
        check_basic(&engine, &module, 9441)?;
        checks.push(mode);
    }
    Ok(serde_json::json!({
        "format_version":2,"milestone":"v0.5-P0-experiment","php_version":"8.2.6",
        "process_pid": std::process::id(),
        "artifact_sha256":FIXTURE_SHA256,"wasmtime_version":"36.0.19",
        "cranelift_opt_level":"none",
        "checks":checks,"recovery_requests":6,"memory_limit_bytes":MEMORY,
        "stdout_limit_bytes":OUTPUT,"fuel_per_request":FUEL,
        "product_ready":false,"wordpress_bootstrapped":false,"real_database_connected":false,
        "wordpress_hook_version":"7.1.3","wordpress_hooks_executed":true,
        "bridge":"fixed-fixture stdout commands/read-only reply file; not the product ABI"
    }))
}

fn main() {
    let trace = std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--trace");
    match verify(trace) {
        Ok(result) => println!("PHP_WASI_HOST_PROBE_RESULT={result}"),
        Err(error) => {
            eprintln!("PHP WASI host probe failed: {error:#}");
            std::process::exit(1);
        }
    }
}

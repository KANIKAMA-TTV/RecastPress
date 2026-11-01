//! Complete bounded fixed CGI requests; callers supply only pinned P0 fixtures.
use super::{LimitedPipe, collect_pipe};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use wasmer::{Engine, Module};
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
    runtime::{module_cache::HashedModuleData, task_manager::tokio::TokioTaskManager},
    virtual_fs::FileSystem,
};

pub(crate) struct FixedRequest {
    pub filename: &'static str,
    pub source: &'static [u8],
    pub environment: Vec<(String, String)>,
    pub extra_mounts: Vec<(&'static str, Arc<dyn FileSystem>)>,
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    fixed: FixedRequest,
    record: &mut Value,
) -> Result<()> {
    let body = execute_response(engine, module, data, fixed, record)?;
    ensure!(
        body.ends_with(b"\n") && body.iter().filter(|&&b| b == b'\n').count() == 1,
        "Fiber CGI body must be one terminated JSON line"
    );
    record["guest_json"] = serde_json::from_slice::<Value>(&body).context("Fiber CGI JSON body")?;
    record["guest_json_decoded"] = json!(true);
    Ok(())
}

/// Capture a complete successful response without imposing a body format.
/// Each closed caller must validate its own body before accepting a request.
pub(crate) fn execute_response(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    fixed: FixedRequest,
    record: &mut Value,
) -> Result<Vec<u8>> {
    record["guest_environment_supplied"] = json!(fixed.environment);
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
        let tasks = TokioTaskManager::new(tokio_runtime.handle().clone());
        let mut runtime = PluggableRuntime::new(Arc::new(tasks));
        runtime.set_engine(engine.clone());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let mut runner = WasiRunner::new();
        runner
            .with_args([
                "-n",
                "-d",
                "fiber.stack_size=2097152",
                "-d",
                "register_argc_argv=0",
            ])
            .with_envs(fixed.environment)
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(true)
            .with_current_dir("/fixture")
            .with_mount(
                "/fixture".into(),
                Arc::new(super::language_contract::single_file_fs(
                    fixed.filename,
                    fixed.source,
                )),
            )
            .with_stdin(Box::new(stdin_reader))
            .with_stdout(Box::new(LimitedPipe::new(
                stdout_sender,
                stdout_overflow.clone(),
            )))
            .with_stderr(Box::new(LimitedPipe::new(
                stderr_sender,
                stderr_overflow.clone(),
            )));
        for (path, mount) in fixed.extra_mounts {
            runner.with_mount(path.into(), mount);
        }
        let caps = runner.capabilities_mut();
        caps.insecure_allow_all = false;
        caps.threading.max_threads = Some(1);
        caps.threading.enable_deep_sleep = false;
        let started = Instant::now();
        let result = runner.run_wasm(
            RuntimeOrEngine::Runtime(Arc::new(runtime)),
            "php-fiber",
            module,
            *data.hash(),
        );
        record["guest_execution_elapsed_ms"] = json!(started.elapsed().as_millis());
        drop(runner);
        result.context("run fixed patched PHP Fiber contract")
    })();
    record["ok"] = json!(outcome.is_ok());
    if let Err(error) = &outcome {
        record["host_execution_error"] =
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
    if let Ok(bytes) = &stdout {
        record["stdout_bytes"] = json!(bytes.len());
        record["stdout_hex"] = json!(hex(bytes));
        record["stdout_sha256"] = json!(sha(bytes));
    }
    if let Ok(bytes) = &stderr {
        record["stderr_bytes"] = json!(bytes.len());
        record["stderr_hex"] = json!(hex(bytes));
    }
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "Fiber output exceeded quota"
    );
    ensure!(
        stderr.context("Fiber stderr EOF")?.is_empty(),
        "Fiber unexpected stderr"
    );
    let stdout = stdout.context("Fiber stdout EOF")?;
    // Parse only the complete successful response, after exit, EOF and quotas.
    let response = super::cgi_contract::cgi_response::parse(&stdout)?;
    record["response"] = json!({"status":response.status,
        "headers":response.headers.iter().map(|(name,value)| json!({"name":name,"value_hex":hex(value)})).collect::<Vec<_>>(),
        "body_bytes":response.body.len(),"body_hex":hex(&response.body),
        "body_sha256":sha(&response.body),"representation_length":response.representation_length});
    ensure!(response.status == 200, "Fiber CGI status");
    Ok(response.body)
}

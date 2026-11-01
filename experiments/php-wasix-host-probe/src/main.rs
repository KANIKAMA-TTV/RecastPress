#![forbid(unsafe_code)]

//! Closed P0 candidates. Run under an owned-child supervisor.
//! A version-only result does not establish exception, WordPress, or DB support.

mod cgi_contract;
mod cgi_execution;
mod cgi_runtime_contract;
mod context_contract;
mod fiber_adapter;
mod fiber_cgi_contract;
mod fiber_contract;
mod language_contract;
mod managed_access_fs;
mod managed_contract;
#[cfg(test)]
mod memory_limits;
mod memory_policy;
#[cfg(test)]
mod metering_limits;
mod module_reuse;
mod php_memory_cap;
mod php_operation_budget;
mod rpc_contract;
mod rpc_fs;
mod runtime_fixture_fs;

use std::fs::File;
use std::io::{self, Read, SeekFrom, Write};
use std::path::Path;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use wasmer::Engine;
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{
    FsError, Pipe, PluggableRuntime, Runtime, UnsupportedVirtualNetworking, VirtualFile,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

const ARTIFACT_RELATIVE: &str = ".reference/php-wasix-8.5.12/php.wasm";
const ARTIFACT_BYTES: u64 = 39_727_549;
const ARTIFACT_SHA256: &str = "e64850fde91d98dff0bce2b95ed467591376586469b1f4d8bad5c8759c4edbe4";
const PHP_SOURCE_COMMIT: &str = "f4bfae79d5c19028349082f707930cf0951d23b9";
const OUTPUT_LIMIT: usize = 64 * 1024;
const ECHO_VALUE: &str = "recastpress-v0.5-wasix-version-only";
const PHP_PROGRAM: &str = "echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,'echo'=>'recastpress-v0.5-wasix-version-only'], JSON_THROW_ON_ERROR);";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeMode {
    VersionOnly,
    LanguageContract,
    ModuleReuse,
    PhpMemoryCap,
    CgiContract,
    RpcContract,
    ContextContract,
    PhpOperationBudget,
    FiberContract,
    FiberCgiContract,
    CgiRuntimeContract,
    ManagedFiberContract,
    ManagedRuntimeContract,
}

#[cfg(feature = "llvm")]
const ENGINE_NAME: &str = "wasmer-7.2.0/llvm";
#[cfg(all(not(feature = "llvm"), feature = "cranelift"))]
const ENGINE_NAME: &str = "wasmer-7.2.0/cranelift";

#[cfg(not(any(feature = "llvm", feature = "cranelift")))]
compile_error!("Enable either the cranelift or llvm feature");

fn main() -> ExitCode {
    // The fixed selectors never accept PHP source, a script path, an
    // environment mapping, or arbitrary PHP arguments from the caller.
    let mode = if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--language-contract"] {
        ProbeMode::LanguageContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--module-reuse"] {
        ProbeMode::ModuleReuse
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--php-memory-cap"] {
        ProbeMode::PhpMemoryCap
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--cgi-contract"] {
        ProbeMode::CgiContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--rpc-contract"] {
        ProbeMode::RpcContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--context-contract"] {
        ProbeMode::ContextContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--php-operation-budget"] {
        ProbeMode::PhpOperationBudget
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--fiber-contract"] {
        ProbeMode::FiberContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--fiber-cgi-contract"] {
        ProbeMode::FiberCgiContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--cgi-runtime-contract"] {
        ProbeMode::CgiRuntimeContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--managed-fiber-cgi-contract"] {
        ProbeMode::ManagedFiberContract
    } else if std::env::args_os().skip(1).collect::<Vec<_>>() == ["--managed-cgi-runtime-contract"]
    {
        ProbeMode::ManagedRuntimeContract
    } else {
        ProbeMode::VersionOnly
    };
    let mut report = json!({
        "schema": "recastpress.php-wasix-version-only.v0.5",
        "ok": false,
        "phase": "initializing",
        "process_pid": std::process::id(),
        "engine": ENGINE_NAME,
        "wasmer_wasix": "0.702.0",
        "host": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "source": {
            "repository": "https://github.com/wasix-org/php",
            "commit": PHP_SOURCE_COMMIT,
            "kind": "official-github-actions-artifact"
        },
        "artifact": {
            "path": ARTIFACT_RELATIVE,
            "expected_bytes": ARTIFACT_BYTES,
            "expected_sha256": ARTIFACT_SHA256,
            "verified": false
        },
        "scope": "fixed-version-only-no-user-code-no-product-integration",
        "real_db": false,
        "wordpress_executed": false,
        "full_php_compatibility_verified": false,
        "exceptions_executed": false,
        "exception_support": {
            "execution_verdict": "not-exercised",
            "msvc_source_verdict": "negative-unimplemented-panic-stubs"
        },
        "integer_width_matches_native": null,
        "isolation": {
            "host_environment_forwarded": false,
            "home_mapped": false,
            "host_directories_mapped": false,
            "networking": "UnsupportedVirtualNetworking",
            "http_client": false,
            "stdin": "closed-pipe",
            "stdout_limit_bytes": OUTPUT_LIMIT,
            "stderr_limit_bytes": OUTPUT_LIMIT,
            "configured_guest_max_threads": 1,
            "memory_limit_enforced": false,
            "in_process_deadline_enforced": false,
            "deadline_owner": "required-external-owned-child-supervisor"
        },
        "limitations": [
            "A basic version result does not verify thrown exceptions or full PHP compatibility.",
            "Wasmer 7.2.0 MSVC exception runtime entry points are unimplemented and may panic.",
            "Memory, CPU and execution deadline enforcement are not established by this probe.",
            "PHP integer width is observed separately from wasm32 linear-memory width."
        ]
    });

    if mode == ProbeMode::LanguageContract {
        language_contract::describe(&mut report);
    } else if mode == ProbeMode::ModuleReuse {
        module_reuse::describe(&mut report);
    } else if mode == ProbeMode::PhpMemoryCap {
        php_memory_cap::describe(&mut report);
    } else if mode == ProbeMode::CgiContract {
        cgi_contract::describe(&mut report);
    } else if mode == ProbeMode::RpcContract {
        rpc_contract::describe(&mut report);
    } else if mode == ProbeMode::ContextContract {
        context_contract::describe(&mut report);
    } else if mode == ProbeMode::PhpOperationBudget {
        php_operation_budget::describe(&mut report);
    } else if mode == ProbeMode::FiberContract {
        fiber_contract::describe(&mut report);
    } else if mode == ProbeMode::FiberCgiContract {
        fiber_cgi_contract::describe(&mut report);
    } else if mode == ProbeMode::CgiRuntimeContract {
        cgi_runtime_contract::describe(&mut report);
    } else if mode == ProbeMode::ManagedFiberContract {
        managed_contract::describe_fiber(&mut report);
    } else if mode == ProbeMode::ManagedRuntimeContract {
        managed_contract::describe_runtime(&mut report);
    }

    let passed = match run(&mut report, mode) {
        Ok(()) => {
            report["ok"] = json!(true);
            report["phase"] = json!(match mode {
                ProbeMode::VersionOnly => "version_only_verified",
                // Observation is not a native comparison. language_check.py
                // compares all fields of the three cases against the receipt.
                ProbeMode::LanguageContract => "language_contract_observed",
                ProbeMode::ModuleReuse => "module_reuse_verified",
                ProbeMode::PhpMemoryCap => "php_memory_cap_diagnostic_verified",
                ProbeMode::CgiContract => "cgi_contract_verified",
                ProbeMode::RpcContract => "rpc_contract_diagnostic_verified",
                ProbeMode::ContextContract => "context_contract_verified",
                ProbeMode::PhpOperationBudget => "php_operation_budget_diagnostic_verified",
                ProbeMode::FiberContract => "fiber_contract_verified",
                ProbeMode::FiberCgiContract => "fiber_cgi_contract_verified",
                ProbeMode::CgiRuntimeContract => "cgi_runtime_contract_verified",
                ProbeMode::ManagedFiberContract => "managed_fiber_contract_verified",
                ProbeMode::ManagedRuntimeContract => "managed_runtime_contract_verified",
            });
            true
        }
        Err(error) => {
            // Keep a failed phase; an ordinary failure must not become an
            // unsupported-platform success. Host aborts are recorded by the
            // external supervisor, since they cannot emit a final JSON here.
            report["error"] = json!(format!("{error:#}").chars().take(4096).collect::<String>());
            false
        }
    };
    let mut stdout = io::stdout().lock();
    if serde_json::to_writer(&mut stdout, &report).is_err()
        || writeln!(&mut stdout).is_err()
        || stdout.flush().is_err()
    {
        return ExitCode::FAILURE;
    }
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(report: &mut Value, mode: ProbeMode) -> Result<()> {
    ensure!(
        (mode == ProbeMode::VersionOnly && std::env::args_os().len() == 1)
            || (mode == ProbeMode::LanguageContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::ModuleReuse && std::env::args_os().len() == 2)
            || (mode == ProbeMode::PhpMemoryCap && std::env::args_os().len() == 2)
            || (mode == ProbeMode::CgiContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::RpcContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::ContextContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::PhpOperationBudget && std::env::args_os().len() == 2)
            || (mode == ProbeMode::FiberContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::FiberCgiContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::CgiRuntimeContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::ManagedFiberContract && std::env::args_os().len() == 2)
            || (mode == ProbeMode::ManagedRuntimeContract && std::env::args_os().len() == 2),
        "this fixed probe accepts only --language-contract, --module-reuse, --php-memory-cap, --cgi-contract, --rpc-contract, --context-contract, --php-operation-budget, --fiber-contract, --fiber-cgi-contract, --cgi-runtime-contract, --managed-fiber-cgi-contract, --managed-cgi-runtime-contract or no arguments"
    );
    if mode == ProbeMode::CgiContract {
        return cgi_contract::verify(report);
    } else if mode == ProbeMode::ContextContract {
        return context_contract::verify(report);
    } else if mode == ProbeMode::FiberContract {
        return fiber_contract::verify(report);
    } else if mode == ProbeMode::FiberCgiContract {
        return fiber_cgi_contract::verify(report);
    } else if mode == ProbeMode::CgiRuntimeContract {
        return cgi_runtime_contract::verify(report);
    } else if mode == ProbeMode::ManagedFiberContract {
        return managed_contract::verify_fiber(report);
    } else if mode == ProbeMode::ManagedRuntimeContract {
        return managed_contract::verify_runtime(report);
    }
    if mode == ProbeMode::LanguageContract {
        language_contract::verify_fixture(report)?;
    }
    report["phase"] = json!("artifact_verification");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(ARTIFACT_RELATIVE);
    let file = File::open(&path).context("open fixed PHP WASIX artifact")?;
    ensure!(
        file.metadata()?.len() == ARTIFACT_BYTES,
        "artifact byte length mismatch"
    );
    let mut bytes = Vec::with_capacity(ARTIFACT_BYTES as usize);
    file.take(ARTIFACT_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == ARTIFACT_BYTES,
        "artifact changed byte length while reading"
    );
    let digest = format!("{:x}", Sha256::digest(&bytes));
    report["artifact"]["observed_sha256"] = json!(digest);
    ensure!(digest == ARTIFACT_SHA256, "artifact SHA-256 mismatch");
    report["artifact"]["verified"] = json!(true);

    if mode == ProbeMode::ModuleReuse {
        return module_reuse::verify(bytes, report);
    } else if mode == ProbeMode::PhpMemoryCap {
        return php_memory_cap::verify(bytes, report);
    } else if mode == ProbeMode::RpcContract {
        return rpc_contract::verify(bytes, report);
    } else if mode == ProbeMode::PhpOperationBudget {
        return php_operation_budget::verify(bytes, report);
    }

    let (stdout, stderr) = execute(bytes, report, mode)?;
    ensure!(stderr.is_empty(), "PHP wrote unexpected stderr");
    report["phase"] = json!("php_json_verification");
    let php: Value =
        serde_json::from_slice(&stdout).context("decode PHP stdout as one JSON value")?;
    report["php"] = php.clone();
    if mode == ProbeMode::LanguageContract {
        return language_contract::validate_observation(&php, report);
    }
    ensure!(php["php_version"] == "8.5.12", "unexpected PHP_VERSION");
    ensure!(php["echo"] == ECHO_VALUE, "fixed PHP echo result mismatch");
    // The official -64 variant can use a 64-bit zend_long in a wasm32 module.
    // Do not infer PHP_INT_SIZE from memory64=false or native host pointers.
    ensure!(
        matches!(php["php_int_size"].as_u64(), Some(4 | 8)),
        "invalid PHP_INT_SIZE"
    );
    Ok(())
}

fn make_engine() -> Engine {
    // LLVM wins when both features are enabled. For the smaller LLVM-only
    // dependency graph use --no-default-features --features llvm.
    #[cfg(feature = "llvm")]
    {
        wasmer_compiler_llvm::LLVM::default().into()
    }
    #[cfg(all(not(feature = "llvm"), feature = "cranelift"))]
    {
        wasmer_compiler_cranelift::Cranelift::default().into()
    }
}

fn execute(bytes: Vec<u8>, report: &mut Value, mode: ProbeMode) -> Result<(Vec<u8>, Vec<u8>)> {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create isolated WASIX runtime")?;
    let guard = tokio_runtime.enter();
    let (stdin_sender, stdin_reader) = Pipe::channel();
    drop(stdin_sender);
    let (stdout_sender, stdout_reader) = Pipe::channel();
    let (stderr_sender, stderr_reader) = Pipe::channel();
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));

    // A failed setup drops captured senders too. The Runner owns no host mount,
    // environment, command mapping, network implementation or HTTP client.
    let outcome = (|| -> Result<()> {
        let tasks = TokioTaskManager::new(tokio_runtime.handle().clone());
        let mut runtime = PluggableRuntime::new(Arc::new(tasks));
        runtime.set_engine(make_engine());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let data = HashedModuleData::new(bytes);
        let module_hash = *data.hash();
        report["phase"] = json!("module_compile");
        eprintln!("PHP_WASIX_PHASE=module_compile");
        let compilation_started = Instant::now();
        let compiled = runtime.load_hashed_module_sync(data, None);
        report["compile_elapsed_ms"] = json!(compilation_started.elapsed().as_millis());
        let module = compiled.context("compile verified formal-EH PHP module")?;
        let mut runner = WasiRunner::new();
        runner
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(false)
            .with_stdin(Box::new(stdin_reader))
            .with_stdout(Box::new(LimitedPipe::new(
                stdout_sender,
                stdout_overflow.clone(),
            )))
            .with_stderr(Box::new(LimitedPipe::new(
                stderr_sender,
                stderr_overflow.clone(),
            )));
        match mode {
            ProbeMode::VersionOnly => {
                runner.with_args(["-n", "-r", PHP_PROGRAM]);
            }
            ProbeMode::LanguageContract => {
                language_contract::configure(&mut runner);
            }
            ProbeMode::ModuleReuse => bail!("module reuse must use its separate execution path"),
            ProbeMode::PhpMemoryCap => {
                bail!("memory cap diagnostic must use its separate execution path")
            }
            ProbeMode::CgiContract => {
                bail!("CGI contract must use its generated artifact execution path")
            }
            ProbeMode::RpcContract => bail!("RPC contract must use its separate execution path"),
            ProbeMode::ContextContract => {
                bail!("context contract must use its generated C artifact execution path")
            }
            ProbeMode::PhpOperationBudget => {
                bail!("operation budget must use its metered execution path")
            }
            ProbeMode::FiberContract => {
                bail!("Fiber contract must use its patched artifact execution path")
            }
            ProbeMode::FiberCgiContract => {
                bail!("Fiber CGI contract must use its combined patched artifact execution path")
            }
            ProbeMode::CgiRuntimeContract => {
                bail!("CGI runtime contract must use its fixed source and mount execution path")
            }
            ProbeMode::ManagedFiberContract | ProbeMode::ManagedRuntimeContract => {
                bail!("managed contracts must use their own closed three-patch artifact path")
            }
        }
        let capabilities = runner.capabilities_mut();
        capabilities.insecure_allow_all = false;
        capabilities.threading.max_threads = Some(1);
        capabilities.threading.enable_deep_sleep = false;
        report["phase"] = json!("fixed_php_execution");
        eprintln!("PHP_WASIX_PHASE=fixed_php_execution");
        // The Runner supplies imported env::memory and initializes WASIX from
        // that memory. Manual WasiFunctionEnv::initialize would incorrectly
        // require an exported memory for this official artifact.
        let execution_started = Instant::now();
        let result = runner.run_wasm(
            RuntimeOrEngine::Runtime(Arc::new(runtime)),
            "php",
            module,
            module_hash,
        );
        // This span includes WASIX/PHP startup and request shutdown inside
        // run_wasm. It excludes module compilation and outer runtime cleanup.
        report["php_execution_elapsed_ms"] = json!(execution_started.elapsed().as_millis());
        drop(runner);
        result.context("execute fixed version-only PHP program")
    })();

    drop(guard);
    // Do not block forever waiting for an idle task during cleanup. The outer
    // supervisor remains responsible for the entire owned process lifetime.
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    if let Ok(bytes) = &stdout {
        report["stdout_bytes"] = json!(bytes.len());
        report["stdout"] = json!(String::from_utf8_lossy(bytes));
    }
    if let Ok(bytes) = &stderr {
        report["stderr_bytes"] = json!(bytes.len());
        report["stderr"] = json!(String::from_utf8_lossy(bytes));
    }
    report["stdout_overflow"] = json!(stdout_overflow.load(Ordering::Relaxed));
    report["stderr_overflow"] = json!(stderr_overflow.load(Ordering::Relaxed));
    // Retain compilation/execution failure as the primary error, but collect
    // both outputs and finish cleanup even when execution failed.
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed),
        "stdout exceeded finite capture limit"
    );
    ensure!(
        !stderr_overflow.load(Ordering::Relaxed),
        "stderr exceeded finite capture limit"
    );
    Ok((
        stdout.context("collect stdout through EOF")?,
        stderr.context("collect stderr through EOF")?,
    ))
}

fn collect_pipe(mut pipe: Pipe) -> Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut collected = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match pipe.try_read(&mut buffer) {
            Some(0) => return Ok(collected),
            Some(count) => {
                ensure!(
                    count <= OUTPUT_LIMIT.saturating_sub(collected.len()),
                    "capture exceeded limit"
                );
                collected.extend_from_slice(&buffer[..count]);
            }
            None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(1)),
            None => bail!("pipe did not close within cleanup deadline"),
        }
    }
}

/// Bounds writes before Pipe's unbounded internal queue can retain them.
/// No reader thread, host file or unsafe runtime hook is required.
#[derive(Debug)]
struct LimitedPipe {
    inner: Pipe,
    written: usize,
    overflow: Arc<AtomicBool>,
}

impl LimitedPipe {
    fn new(inner: Pipe, overflow: Arc<AtomicBool>) -> Self {
        Self {
            inner,
            written: 0,
            overflow,
        }
    }
}

impl AsyncRead for LimitedPipe {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for LimitedPipe {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if buf.len() > OUTPUT_LIMIT.saturating_sub(self.written) {
            self.overflow.store(true, Ordering::Relaxed);
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "fixed probe output limit exceeded",
            )));
        }
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(count)) => {
                self.written += count;
                Poll::Ready(Ok(count))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl AsyncSeek for LimitedPipe {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        Pin::new(&mut self.inner).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<u64>> {
        Pin::new(&mut self.inner).poll_complete(cx)
    }
}

impl VirtualFile for LimitedPipe {
    fn last_accessed(&self) -> u64 {
        self.inner.last_accessed()
    }
    fn last_modified(&self) -> u64 {
        self.inner.last_modified()
    }
    fn created_time(&self) -> u64 {
        self.inner.created_time()
    }
    fn size(&self) -> u64 {
        self.inner.size()
    }
    fn set_len(&mut self, size: u64) -> std::result::Result<(), FsError> {
        self.inner.set_len(size)
    }
    fn unlink(&mut self) -> std::result::Result<(), FsError> {
        self.inner.unlink()
    }
    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
    fn poll_read_ready(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_read_ready(cx)
    }
    fn poll_write_ready(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write_ready(cx) {
            Poll::Ready(Ok(count)) => {
                Poll::Ready(Ok(count.min(OUTPUT_LIMIT.saturating_sub(self.written))))
            }
            other => other,
        }
    }
}

#![forbid(unsafe_code)]

//! Compiler diagnostics only: never instantiate a module or execute a guest.

use std::fs;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::{Engine, Module};
use wasmer_compiler_cranelift::Cranelift;

const NON_EH: &[u8] = include_bytes!("../fixtures/non-eh.wat");
const TRY_TABLE_CALL: &[u8] = include_bytes!("../fixtures/try-table-call.wat");
const STREAM_LIMIT: u64 = 64 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(10);
const REAP_TIMEOUT: Duration = Duration::from_secs(2);
const CAPTURE_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
const CRANELIFT_SOURCE: &str = "423be7a4e4d30bb377c836d317521d1eb874e157";
const WASMER_SOURCE: &str = "93bd021012baafda53df2030db3ce1758dc1e336";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [] => supervise(),
        [child, name] if child == "--child" => compile_child(name),
        _ => Err(anyhow::anyhow!(
            "use no arguments, or internal --child CASE"
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn compile_child(name: &str) -> Result<()> {
    let source = match name {
        "non-eh" => NON_EH,
        "try-table-call" => TRY_TABLE_CALL,
        _ => bail!("unknown fixed compile-only case"),
    };
    // Parsing must finish before entering the compiler. A malformed fixture
    // therefore cannot be mistaken for a Cranelift code-generation failure.
    let wasm = match wasmer::wat2wasm(source) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!(
                "{}",
                json!({"case": name, "kind": "fixture-parse-error", "error": error.to_string()})
            );
            return Ok(());
        }
    };
    let mut compiler = Cranelift::default();
    compiler.num_threads(NonZeroUsize::new(1).expect("one is nonzero"));
    let engine: Engine = compiler.into();
    let mut report = json!({
        "case": name,
        "phase": "module_compile",
        "engine": "wasmer-7.2.0/cranelift",
        "compiler_threads": 1,
        "wat_sha256": sha256(source),
        "wasm_sha256": sha256(&wasm),
        "wasm_bytes": wasm.len(),
        "guest_instantiated": false,
        "guest_executed": false
    });
    // Intentionally do not catch_unwind: a backend panic must retain its real
    // exit status and panic diagnostic in this separate owned child process.
    match Module::new(&engine, &wasm[..]) {
        Ok(_) => report["kind"] = json!("compiled"),
        Err(error) => {
            report["kind"] = json!("compile-error");
            report["error"] = json!(error.to_string());
        }
    }
    println!("{report}");
    Ok(())
}

fn supervise() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let executable = std::env::current_exe()?.canonicalize()?;
    let binary = json!({
        "path": executable.strip_prefix(&root)
            .context("diagnostic binary must be within the workspace")?
            .to_string_lossy().replace('\\', "/"),
        "bytes": fs::metadata(&executable)?.len(),
        "sha256": sha256_file(&executable)?,
        "hashed_before_spawn": true
    });
    let raw_dir = root.join(".reference/php-wasix-eh-compile-probe");
    fs::create_dir_all(&raw_dir)?;
    let mut cases = Vec::new();
    // Each case gets a new PID and fresh compiler state. A panic in the EH
    // case cannot suppress or contaminate the non-EH baseline result.
    for name in ["non-eh", "try-table-call"] {
        cases.push(run_owned_child(name, &raw_dir, &executable)?);
    }
    let baseline_compiled = cases[0]["kind"] == "compiled";
    let both_classified = cases.iter().all(|case| {
        matches!(
            case["kind"].as_str(),
            Some("compiled" | "compile-error" | "rust-panic")
        )
    });
    let report = json!({
        "schema": "recastpress.php-wasix-eh-compile-diagnostic.v0.5",
        "scope": "compile-only-backend-diagnostic",
        "diagnostic_complete": baseline_compiled && both_classified,
        "product_compatibility_verified": false,
        "php_executed": false,
        "guest_instantiated": false,
        "guest_executed": false,
        "run_unix_seconds": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        "host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH, "environment": target_env()},
        "dependencies": {"wasmer": "7.2.0", "wasmer-compiler-cranelift": "7.2.0", "cranelift-codegen": "0.133.0"},
        "compiler_settings": {"threads": 1, "optimization": "default-speed", "exceptions": "backend-default"},
        "owned_child_deadline_seconds": CHILD_TIMEOUT.as_secs(),
        "owned_child_reap_deadline_seconds": REAP_TIMEOUT.as_secs(),
        "capture_cleanup_deadline_seconds": CAPTURE_CLEANUP_TIMEOUT.as_secs(),
        "captured_stream_limit_bytes": STREAM_LIMIT,
        "binary": binary,
        "source_sha256": {
            "Cargo.toml": sha256(include_bytes!("../Cargo.toml")),
            "Cargo.lock": sha256(include_bytes!("../Cargo.lock")),
            "src/main.rs": sha256(include_bytes!("main.rs")),
            "fixtures/non-eh.wat": sha256(NON_EH),
            "fixtures/try-table-call.wat": sha256(TRY_TABLE_CALL)
        },
        "source_evidence": {
            "cranelift_git_commit": CRANELIFT_SOURCE,
            "wasmer_git_commit": WASMER_SOURCE,
            "cranelift_payload_types": format!("https://github.com/bytecodealliance/wasmtime/blob/{CRANELIFT_SOURCE}/cranelift/codegen/src/isa/call_conv.rs#L118-L128"),
            "cranelift_original_php_panic_index": format!("https://github.com/bytecodealliance/wasmtime/blob/{CRANELIFT_SOURCE}/cranelift/codegen/src/machinst/lower.rs#L1105-L1108"),
            "cranelift_minimal_panic_site": format!("https://github.com/bytecodealliance/wasmtime/blob/{CRANELIFT_SOURCE}/cranelift/codegen/src/isa/x64/abi.rs#L888-L904"),
            "cranelift_payload_registers": format!("https://github.com/bytecodealliance/wasmtime/blob/{CRANELIFT_SOURCE}/cranelift/codegen/src/isa/x64/abi.rs#L979-L986"),
            "wasmer_payload_arguments": format!("https://github.com/wasmerio/wasmer/blob/{WASMER_SOURCE}/lib/compiler-cranelift/src/func_environ.rs#L1215-L1229")
        },
        "interpretation": "Compiled means only backend compilation completed; CompileError and panic remain negative backend outcomes. No guest, PHP, WASIX runtime, WordPress or product compatibility was tested.",
        "cases": cases
    });
    let evidence_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("evidence");
    fs::create_dir_all(&evidence_dir)?;
    let evidence_file = evidence_dir.join(format!(
        "{}-{}-{}.json",
        std::env::consts::OS,
        std::env::consts::ARCH,
        target_env()
    ));
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    fs::write(evidence_file, bytes)?;
    println!("{report}");
    ensure!(baseline_compiled, "non-EH baseline did not compile");
    ensure!(
        both_classified,
        "at least one child could not be classified"
    );
    Ok(())
}

fn run_owned_child(name: &str, raw_dir: &Path, executable: &Path) -> Result<Value> {
    let started = Instant::now();
    let mut child = Command::new(executable)
        .args(["--child", name])
        .env("RUST_BACKTRACE", "0")
        .env("RUST_LIB_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn owned compiler child {name}"))?;
    let pid = child.id();
    let stdout = capture(child.stdout.take().context("owned stdout missing")?);
    let stderr = capture(child.stderr.take().context("owned stderr missing")?);
    let mut timed_out = false;
    let mut wait_error = None;
    let mut status: Option<ExitStatus> = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() < CHILD_TIMEOUT => {
                thread::sleep(Duration::from_millis(5))
            }
            Ok(None) => {
                timed_out = true;
                break None;
            }
            Err(error) => {
                wait_error = Some(error.to_string());
                break None;
            }
        }
    };
    let kill_attempted = status.is_none();
    let mut kill_error = None;
    if kill_attempted {
        kill_error = child.kill().err().map(|error| error.to_string());
        // A failed kill must not turn into an unbounded wait. Continue to
        // observe this owned PID for a separate finite cleanup interval.
        let reap_deadline = Instant::now() + REAP_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    status = Some(exit);
                    break;
                }
                Ok(None) => {}
                Err(error) => wait_error = Some(error.to_string()),
            }
            if Instant::now() >= reap_deadline {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    // A reader can remain blocked if a child cannot be killed or reaped.
    // Join only after completion is observed, under one shared deadline.
    // If cleanup is unconfirmed, retain that negative result and its PID.
    let capture_deadline = Instant::now() + CAPTURE_CLEANUP_TIMEOUT;
    let (stdout_finished, stdout_result) = join_capture(stdout, capture_deadline);
    let (stderr_finished, stderr_result) = join_capture(stderr, capture_deadline);
    let stdout_error = stdout_result
        .as_ref()
        .err()
        .map(|error| redact(&error.to_string()));
    let stderr_error = stderr_result
        .as_ref()
        .err()
        .map(|error| redact(&error.to_string()));
    let stdout = stdout_result.unwrap_or_default();
    let stderr = stderr_result.unwrap_or_default();
    fs::write(raw_dir.join(format!("{name}.stdout")), &stdout)?;
    fs::write(raw_dir.join(format!("{name}.stderr")), &stderr)?;
    let stdout_text = redact(&String::from_utf8_lossy(&stdout));
    let stderr_text = redact(&String::from_utf8_lossy(&stderr));
    let parsed: Option<Value> = serde_json::from_slice(&stdout).ok();
    let stream_overflow = stdout.len() as u64 > STREAM_LIMIT || stderr.len() as u64 > STREAM_LIMIT;
    let exit_success = status.as_ref().is_some_and(ExitStatus::success);
    let kind = if status.is_none() {
        "unreaped-child"
    } else if wait_error.is_some() {
        "process-control-error"
    } else if !stdout_finished
        || !stderr_finished
        || stdout_error.is_some()
        || stderr_error.is_some()
    {
        "capture-incomplete"
    } else if timed_out {
        "timeout"
    } else if stream_overflow {
        "stream-overflow"
    } else if !exit_success && stderr_text.contains("panicked at") {
        "rust-panic"
    } else if exit_success {
        match parsed.as_ref().and_then(|value| value["kind"].as_str()) {
            Some("compiled") => "compiled",
            Some("compile-error") => "compile-error",
            Some("fixture-parse-error") => "fixture-parse-error",
            _ => "invalid-child-output",
        }
    } else {
        "abnormal-exit"
    };
    Ok(json!({
        "case": name,
        "kind": kind,
        "process_pid": pid,
        "exit_code": status.as_ref().and_then(ExitStatus::code),
        "exit_success": exit_success,
        "reaped": status.is_some(),
        "kill_attempted": kill_attempted,
        "kill_error": kill_error.map(|error| redact(&error)),
        "wait_error": wait_error.map(|error| redact(&error)),
        "timed_out": timed_out,
        "elapsed_ms": started.elapsed().as_millis(),
        "stdout": stdout_text,
        "stderr": stderr_text,
        "stdout_reader_finished": stdout_finished,
        "stderr_reader_finished": stderr_finished,
        "stdout_capture_error": stdout_error,
        "stderr_capture_error": stderr_error,
        "wat_sha256": sha256(if name == "non-eh" { NON_EH } else { TRY_TABLE_CALL }),
        "child_report": parsed.map(redact_value)
    }))
}

fn capture(stream: impl Read + Send + 'static) -> JoinHandle<Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        stream.take(STREAM_LIMIT + 1).read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn join_capture(handle: JoinHandle<Result<Vec<u8>>>, deadline: Instant) -> (bool, Result<Vec<u8>>) {
    while !handle.is_finished() {
        if Instant::now() >= deadline {
            return (
                false,
                Err(anyhow::anyhow!(
                    "owned child stream reader did not finish before cleanup deadline"
                )),
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
    (
        true,
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("owned child stream reader panicked"))
            .and_then(|result| result),
    )
}

fn redact(text: &str) -> String {
    let mut text = text.replace('\\', "/");
    for key in ["CARGO_HOME", "USERPROFILE", "HOME"] {
        if let Some(path) = std::env::var_os(key) {
            let normalized = path.to_string_lossy().replace('\\', "/");
            if !normalized.is_empty() {
                text = text.replace(
                    &normalized,
                    if key == "CARGO_HOME" {
                        "<cargo-home>"
                    } else {
                        "<user-home>"
                    },
                );
            }
        }
    }
    text
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::String(text) => json!(redact(&text)),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, redact_value(value)))
                .collect(),
        ),
        value => value,
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn target_env() -> &'static str {
    if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else {
        "other"
    }
}

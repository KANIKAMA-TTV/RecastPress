//! Three closed CGI requests against this CI job's verified build output.
//! This is not an arbitrary CGI gateway, product worker or binary distribution.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::{Engine, Module};
use wasmer_wasix::runtime::module_cache::HashedModuleData;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{
    Pipe, PluggableRuntime, UnsupportedVirtualNetworking,
    runners::wasi::{RuntimeOrEngine, WasiRunner},
};

use super::{LimitedPipe, collect_pipe, make_engine};

// Reuse the complete-response parser unchanged, including its nine unit tests.
#[path = "../../php-wasi-host-probe/src/cgi_response.rs"]
pub(crate) mod cgi_response;

const BUILD_RECEIPT: &str = "experiments/php-wasix-cgi-build/evidence/build.json";
const BUILD_MANIFEST: &str = "experiments/php-wasix-cgi-build/manifest.json";
const MODULE_PATH: &str = "target/php-wasix-cgi-build/source/php/sapi/cgi/php-cgi.wasm";
const SOURCE: &str = "f4bfae79d5c19028349082f707930cf0951d23b9";
const MANIFEST: &[u8] = include_bytes!("../../php-wasix-cgi-build/manifest.json");
const FIXTURE: &[u8] = include_bytes!("../fixtures/cgi_contract.php");
const FIXTURE_SHA: &str = "ac0e2fb549a7fb0c8620effcbe7cc40dd84f171f0b6dfad639f1fafb65e51bc0";
const GUEST_PATH: &str = "/fixture/cgi_contract.php";
const COOKIE: &str = "wp_logged_in=fixture%20user; wp_test=1";
const POST: &[u8] = b"title=RP%20CGI&count=2&empty=";
const QUERIES: [&str; 3] = [
    "case=get&value=hello%20wasix",
    "case=post&source=fixed",
    "case=metadata",
];
const NAMES: [&str; 3] = ["get_cookie", "post_raw_body", "status_cookies_binary"];

fn sha(wire: &[u8]) -> String {
    format!("{:x}", Sha256::digest(wire))
}

fn hex(wire: &[u8]) -> String {
    wire.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn describe(report: &mut Value) {
    report["schema"] = json!("recastpress.php-wasix-cgi-contract.v0.5");
    report["scope"] = json!("same-job-generated-CGI-three-fixed-requests-no-product-integration");
    report["source"] = json!({"repository": "https://github.com/wasix-org/php", "commit": SOURCE,
                              "kind": "same-job-source-build-with-explicit-prefork-patch"});
    report["artifact"] = json!({"path": MODULE_PATH, "verified": false,
                                "identity_policy": "current-build-receipt-not-CLI-static-artifact-pin"});
    report["build_receipt"] = json!({"path": BUILD_RECEIPT, "verified": false,
        "full_source_tool_config_chain_verified_by_host": false,
        "chain_owner": "cgi_check.py; must compare this host's receipt/module digests with its verified snapshot"});
    report["fixture"] = json!({"source": "experiments/php-wasix-host-probe/fixtures/cgi_contract.php",
        "guest_path": GUEST_PATH, "expected_sha256": FIXTURE_SHA, "verified": false});
    report["isolation"]["guest_fixture_files"] = json!([GUEST_PATH]);
    report["isolation"]["guest_readonly_mounts"] = json!(["/fixture"]);
    report["isolation"]["other_virtual_root_writes_forbidden"] = json!(false);
    report["isolation"]["stdin"] = json!("fixed-CGI-request-body-then-EOF");
    report["isolation"]["memory_limit_enforced"] = json!(false);
    report["native_comparison_performed"] = json!(false);
    report["guest_arguments"] = json!(["-n"]);
    report["cgi_contract_verified"] = json!(false);
    report["reuse"] = json!({"requested_requests": 3, "php_module_compile_invocations": 0,
        "module_clone_count": 0, "completed_requests": 0, "request_path_module_compile_calls": 0,
        "module_delivery": "Module.clone", "counter_scope": "explicit PHP Module::new call site only"});
    report["requests"] = json!([]);
    report["limitations"] = json!([
        "This receipt records a generated candidate; it is not a reproducible-build or signed-attestation claim.",
        "Only three fixed single-request CGI executions are checked; FastCGI, HTTP sockets, WordPress and DB are not exercised.",
        "Fresh request runtimes and pipe EOF do not prove a product worker or complete in-process task cancellation.",
        "No guest linear-memory cap, process RSS cap, CPU budget or in-process execution deadline is established here.",
        "The external owned-child supervisor must confirm host exit and reap before accepting this result."
    ]);
}

/// No caller path is accepted. Resolve every parent without allowing symlinks,
/// bound the read, and hash exactly the bytes subsequently used by the host.
fn read_fixed(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = root.join(name);
    ensure!(
        path.canonicalize()? == path,
        "fixed CGI input resolves through a symlink"
    );
    let file = File::open(&path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= limit,
        "fixed CGI input size/type"
    );
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == metadata.len() && bytes.len() as u64 <= limit,
        "fixed CGI input changed while reading"
    );
    Ok(bytes)
}

fn load_module(report: &mut Value) -> Result<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let manifest_wire = read_fixed(&root, BUILD_MANIFEST, 1024 * 1024)?;
    ensure!(
        manifest_wire == MANIFEST,
        "CGI build manifest changed since host compilation"
    );
    let manifest: Value = serde_json::from_slice(&manifest_wire)?;
    let wire = read_fixed(&root, BUILD_RECEIPT, 16 * 1024 * 1024)?;
    let receipt: Value = serde_json::from_slice(&wire)?;
    report["build_receipt"]["observed_sha256"] = json!(sha(&wire));
    report["build_receipt"]["manifest_sha256"] = json!(sha(&manifest_wire));
    ensure!(
        receipt["format_version"] == 1
            && receipt["outcome"] == "cgi_candidate_built_not_executed"
            && receipt["phase"] == "build_complete"
            && receipt["guest_executed"] == false,
        "CGI build did not finish successfully"
    );
    for name in ["php", "deps", "parser"] {
        ensure!(
            receipt["sources"][name]["commit"] == manifest["sources"][name]["commit"],
            "CGI source commit mismatch"
        );
    }
    ensure!(
        receipt["sources"]["php"]["commit"] == SOURCE,
        "unexpected CGI PHP source"
    );
    let patches = receipt["source_patches"]
        .as_array()
        .context("missing source patches")?;
    let pins = manifest["source_patches"]
        .as_array()
        .context("missing patch pins")?;
    ensure!(
        patches.len() == 1 && pins.len() == 1,
        "unexpected CGI patch set"
    );
    let patch = &patches[0];
    for field in [
        "path",
        "sha256",
        "source_commit",
        "target",
        "before_bytes",
        "before_sha256",
        "after_sha256",
    ] {
        ensure!(
            patch[field] == pins[0][field],
            "CGI patch identity mismatch: {field}"
        );
    }
    ensure!(
        patch["applied"] == true
            && patch["clean_tracked_source_before_patch"] == true
            && patch["observed_patch_sha256"] == pins[0]["sha256"]
            && patch["observed_before_sha256"] == pins[0]["before_sha256"]
            && patch["observed_after_sha256"] == pins[0]["after_sha256"],
        "CGI source transformation was not verified"
    );
    let steps = receipt["build_steps"]
        .as_array()
        .context("missing build steps")?;
    ensure!(steps.len() == 3, "unexpected CGI build step count");
    for (step, name) in steps.iter().zip(["configure", "make_cgi", "optimize_cgi"]) {
        ensure!(
            step["name"] == name && step["completed"] == true && step["exit_code"] == 0,
            "CGI build step incomplete"
        );
    }
    ensure!(
        receipt["cgi_wasm"]["path"] == "sapi/cgi/php-cgi.wasm"
            && receipt["cgi_wasm"]["wasm_header_verified"] == true,
        "unexpected CGI generated path"
    );
    let bytes = read_fixed(&root, MODULE_PATH, 256 * 1024 * 1024)?;
    let hash = sha(&bytes);
    ensure!(
        receipt["cgi_wasm"]["bytes"].as_u64() == Some(bytes.len() as u64)
            && receipt["cgi_wasm"]["sha256"] == hash,
        "CGI generated module identity mismatch"
    );
    ensure!(
        bytes.starts_with(b"\0asm\x01\0\0\0"),
        "CGI output is not WebAssembly v1"
    );
    report["artifact"]["observed_bytes"] = json!(bytes.len());
    report["artifact"]["observed_sha256"] = json!(hash);
    report["artifact"]["verified"] = json!(true);
    report["build_receipt"]["verified"] = json!(true);
    Ok(bytes)
}

pub fn verify(report: &mut Value) -> Result<()> {
    verify_candidate(report, load_module)
}

pub(crate) fn verify_candidate(
    report: &mut Value,
    loader: fn(&mut Value) -> Result<Vec<u8>>,
) -> Result<()> {
    report["phase"] = json!("cgi_build_verification");
    ensure!(sha(FIXTURE) == FIXTURE_SHA, "CGI fixture hash mismatch");
    report["fixture"]["verified"] = json!(true);
    report["fixture"]["observed_sha256"] = json!(sha(FIXTURE));
    let data = HashedModuleData::new(loader(report)?);
    let engine = make_engine();
    report["phase"] = json!("module_compile");
    eprintln!("PHP_WASIX_PHASE=module_compile");
    report["reuse"]["php_module_compile_invocations"] = json!(1);
    let started = Instant::now();
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile same-job CGI module once")?;
    for (index, name) in NAMES.iter().enumerate() {
        report["phase"] = json!(format!("cgi_request_{}", index + 1));
        eprintln!("PHP_WASIX_PHASE=cgi_request_{}", index + 1);
        report["reuse"]["module_clone_count"] = json!(index + 1);
        let mut request = json!({"name": name, "request_number": index + 1,
            "module_compile_invocations": 0, "module_delivery": "Module.clone",
            "php_completed": false, "contract_verified": false});
        let outcome = execute_request(&engine, module.clone(), &data, index, &mut request)
            .and_then(|stdout| validate_response(&stdout, index, &mut request));
        if let Err(error) = &outcome {
            request["error"] = json!(format!("{error:#}").chars().take(4096).collect::<String>());
        }
        report["requests"]
            .as_array_mut()
            .context("requests not initialized")?
            .push(request);
        outcome?;
        report["reuse"]["completed_requests"] = json!(index + 1);
    }
    report["cgi_contract_verified"] = json!(true);
    Ok(())
}

fn environment(index: usize) -> Vec<(String, String)> {
    let method = if index == 1 { "POST" } else { "GET" };
    let body = if index == 1 { POST } else { b"" };
    let mut env: Vec<(String, String)> = [
        ("GATEWAY_INTERFACE", "CGI/1.1"),
        ("SERVER_PROTOCOL", "HTTP/1.1"),
        ("SERVER_SOFTWARE", "recastpress-v0.5-fixed-cgi-probe"),
        ("SERVER_NAME", "fixture.invalid"),
        ("SERVER_PORT", "80"),
        ("REDIRECT_STATUS", "1"),
        ("REQUEST_METHOD", method),
        ("SCRIPT_FILENAME", GUEST_PATH),
        ("SCRIPT_NAME", "/cgi_contract.php"),
        ("DOCUMENT_ROOT", "/fixture"),
        ("QUERY_STRING", QUERIES[index]),
        ("HTTP_COOKIE", COOKIE),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect();
    env.push((
        "REQUEST_URI".into(),
        format!("/cgi_contract.php?{}", QUERIES[index]),
    ));
    env.push(("CONTENT_LENGTH".into(), body.len().to_string()));
    if index == 1 {
        env.push((
            "CONTENT_TYPE".into(),
            "application/x-www-form-urlencoded".into(),
        ));
    }
    env
}

fn execute_request(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    index: usize,
    request: &mut Value,
) -> Result<Vec<u8>> {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let guard = tokio_runtime.enter();
    let (mut stdin_sender, stdin_reader) = Pipe::channel();
    let body = if index == 1 { POST } else { b"" };
    stdin_sender.write_all(body)?;
    drop(stdin_sender);
    let (stdout_sender, stdout_reader) = Pipe::channel();
    let (stderr_sender, stderr_reader) = Pipe::channel();
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let env = environment(index);
    request["guest_environment_supplied"] = json!(env);
    request["stdin_bytes"] = json!(body.len());
    request["stdin_hex"] = json!(hex(body));
    let outcome = (|| -> Result<()> {
        let tasks = TokioTaskManager::new(tokio_runtime.handle().clone());
        let mut runtime = PluggableRuntime::new(Arc::new(tasks));
        runtime.set_engine(engine.clone());
        runtime.set_networking_implementation(UnsupportedVirtualNetworking::default());
        runtime.http_client = None;
        let mut runner = WasiRunner::new();
        runner
            .with_args(["-n"])
            .with_envs(env)
            .with_forward_host_env(false)
            .with_home_mapped(false)
            .with_asynchronous_threading(false)
            .with_current_dir("/fixture")
            .with_mount(
                "/fixture".into(),
                Arc::new(super::language_contract::single_file_fs(
                    "cgi_contract.php",
                    FIXTURE,
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
        let capabilities = runner.capabilities_mut();
        capabilities.insecure_allow_all = false;
        capabilities.threading.max_threads = Some(1);
        capabilities.threading.enable_deep_sleep = false;
        let started = Instant::now();
        let result = runner.run_wasm(
            RuntimeOrEngine::Runtime(Arc::new(runtime)),
            "php-cgi",
            module,
            *data.hash(),
        );
        request["php_execution_elapsed_ms"] = json!(started.elapsed().as_millis());
        request["php_completed"] = json!(result.is_ok());
        drop(runner);
        result.context("execute fixed CGI request and wait for guest exit zero")
    })();
    drop(guard);
    tokio_runtime.shutdown_timeout(Duration::from_secs(2));
    request["runtime_shutdown_returned"] = json!(true);
    request["runtime_shutdown_wait_limit_ms"] = json!(2000);
    let stdout = collect_pipe(stdout_reader);
    let stderr = collect_pipe(stderr_reader);
    request["stdout_eof"] = json!(stdout.is_ok());
    request["stderr_eof"] = json!(stderr.is_ok());
    request["stdout_overflow"] = json!(stdout_overflow.load(Ordering::Relaxed));
    request["stderr_overflow"] = json!(stderr_overflow.load(Ordering::Relaxed));
    if let Ok(bytes) = &stdout {
        request["stdout_bytes"] = json!(bytes.len());
        request["stdout_hex"] = json!(hex(bytes));
        request["stdout_sha256"] = json!(sha(bytes));
    }
    if let Ok(bytes) = &stderr {
        request["stderr_bytes"] = json!(bytes.len());
        request["stderr_hex"] = json!(hex(bytes));
    }
    outcome?;
    ensure!(
        !stdout_overflow.load(Ordering::Relaxed) && !stderr_overflow.load(Ordering::Relaxed),
        "CGI capture exceeded limit"
    );
    let stdout = stdout.context("CGI stdout EOF")?;
    ensure!(
        stderr.context("CGI stderr EOF")?.is_empty(),
        "CGI wrote unexpected stderr"
    );
    Ok(stdout)
}

fn validate_response(stdout: &[u8], index: usize, request: &mut Value) -> Result<()> {
    // Only reached after normal guest exit, runtime shutdown, EOF and budgets.
    let response = cgi_response::parse(stdout)?;
    request["response"] = json!({"status": response.status,
        "headers": response.headers.iter().map(|(name, value)| json!({"name": name, "value_hex": hex(value)})).collect::<Vec<_>>(),
        "body_bytes": response.body.len(), "body_hex": hex(&response.body),
        "body_sha256": sha(&response.body), "representation_length": response.representation_length});
    let values = |name: &str| {
        response
            .headers
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_slice())
            .collect::<Vec<_>>()
    };
    if index == 2 {
        ensure!(response.status == 404, "CGI status mismatch");
        ensure!(
            values("content-type") == [b"application/octet-stream".as_slice()],
            "CGI binary content type mismatch"
        );
        ensure!(
            values("set-cookie")
                == [
                    b"wp_a=one; path=/; HttpOnly".as_slice(),
                    b"wp_b=two; path=/; SameSite=Lax".as_slice()
                ],
            "CGI multi-cookie mismatch"
        );
        ensure!(response.body == b"\0\xffCGI\r\n", "CGI binary body changed");
    } else {
        ensure!(
            response.status == 200 && values("set-cookie").is_empty(),
            "CGI JSON status/cookie mismatch"
        );
        ensure!(
            values("content-type") == [b"application/json".as_slice()],
            "CGI JSON content type mismatch"
        );
        let php: Value = serde_json::from_slice(&response.body).context("CGI PHP JSON body")?;
        request["php"] = php.clone();
        let get = if index == 0 {
            json!({"case": "get", "value": "hello wasix"})
        } else {
            json!({"case": "post", "source": "fixed"})
        };
        let post = if index == 1 {
            json!({"title": "RP CGI", "count": "2", "empty": ""})
        } else {
            json!([])
        };
        let expected = json!({"runtime": {"version": "8.5.12", "version_id": 80512, "int_size": 8, "sapi": "cgi-fcgi"},
            "method": if index == 1 { "POST" } else { "GET" }, "query_string": QUERIES[index],
            "script_filename": GUEST_PATH, "request_uri": format!("/cgi_contract.php?{}", QUERIES[index]),
            "get": get, "post": post, "cookie": {"wp_logged_in": "fixture user", "wp_test": "1"},
            "raw_body_hex": hex(if index == 1 { POST } else { b"" })});
        ensure!(
            php == expected,
            "CGI request inputs/runtime observation differ from the fixed contract"
        );
    }
    request["contract_verified"] = json!(true);
    Ok(())
}

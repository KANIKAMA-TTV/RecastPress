//! Four unchanged private RPC cases on the closed three-patch CGI candidate.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use wasmer::{Engine, Module};
use wasmer_wasix::runtime::module_cache::HashedModuleData;

use super::rpc_contract::{self, ENV_KEY, INCOMPLETE, LITERAL, MARKERS, MOUNT, NORMAL};
use super::rpc_fs::RpcFs;

const FIXTURE: &[u8] =
    include_bytes!("../../php-wasix-managed-access-probe/fixtures/rpc_cgi_contract.php");
const FIXTURE_SHA: &str = "382d54a93357a480926255d7af8d69dadcedaabd34e6dade030ba932abdf5871";
const SOURCE: &str = "experiments/php-wasix-managed-access-probe/fixtures/rpc_cgi_contract.php";
const FILENAME: &str = "rpc_cgi_contract.php";
const GUEST: &str = "/fixture/rpc_cgi_contract.php";

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn describe_fixture(report: &mut Value) {
    report["fixture"] = json!({"source": SOURCE,"guest_path":GUEST,
        "expected_sha256":FIXTURE_SHA,"verified":false,
        "canonical_source":"experiments/php-wasix-host-probe/src/rpc_contract.rs",
        "case_bodies_byte_identical":false});
    report["isolation"]["guest_fixture_files"] = json!([GUEST]);
    report["isolation"]["guest_readonly_mounts"] = json!(["/fixture"]);
    report["isolation"]["guest_environment_policy"] =
        json!("fixed CGI scaffold, one per-request marker and closed normal/incomplete case");
}

pub fn verify(report: &mut Value) -> Result<()> {
    report["phase"] = json!("managed_rpc_fixture_verification");
    ensure!(sha(FIXTURE) == FIXTURE_SHA, "fixed CGI RPC fixture hash");
    for body in [NORMAL, INCOMPLETE] {
        ensure!(
            FIXTURE
                .windows(body.len())
                .filter(|window| *window == body.as_bytes())
                .count()
                == 1,
            "original RPC body must occur exactly once in CGI fixture"
        );
    }
    report["fixture"]["verified"] = json!(true);
    report["fixture"]["observed_sha256"] = json!(sha(FIXTURE));
    report["fixture"]["case_bodies_byte_identical"] = json!(true);
    let bytes = super::managed_contract::load_for_rpc(report)?;
    rpc_contract::verify_with_executor(bytes, report, execute)?;
    ensure!(
        report["observed_php_int_size"] == 8,
        "managed CGI integer width"
    );
    Ok(())
}

fn environment(index: usize) -> Vec<(String, String)> {
    [
        ("GATEWAY_INTERFACE", "CGI/1.1"),
        ("SERVER_PROTOCOL", "HTTP/1.1"),
        ("SERVER_SOFTWARE", "recastpress-v0.5-fixed-rpc-cgi-probe"),
        ("SERVER_NAME", "fixture.invalid"),
        ("SERVER_PORT", "80"),
        ("REDIRECT_STATUS", "200"),
        ("REQUEST_METHOD", "GET"),
        ("SCRIPT_FILENAME", GUEST),
        ("SCRIPT_NAME", "/__rp_rpc.php"),
        ("DOCUMENT_ROOT", "/fixture"),
        ("QUERY_STRING", "rp_rpc_cgi=1"),
        ("REQUEST_URI", "/__rp_rpc.php?rp_rpc_cgi=1"),
        ("CONTENT_LENGTH", "0"),
        (ENV_KEY, MARKERS[index]),
        (
            "RP_RPC_CASE",
            if index == 2 {
                "incomplete_ignored"
            } else {
                "normal"
            },
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect()
}

fn execute(
    engine: &Engine,
    module: Module,
    data: &HashedModuleData,
    index: usize,
    record: &mut Value,
) -> Result<()> {
    let fs = RpcFs::new(101 + index as u64);
    record["fresh_wasi_runtime"] = json!(true);
    record["fresh_rpc_state"] = json!(true);
    record["program_sha256"] = json!(sha(FIXTURE));
    record["guest_arguments"] = json!([
        "-n",
        "-d",
        "fiber.stack_size=2097152",
        "-d",
        "register_argc_argv=0"
    ]);
    let execution = super::cgi_execution::execute_response(
        engine,
        module,
        data,
        super::cgi_execution::FixedRequest {
            filename: FILENAME,
            source: FIXTURE,
            environment: environment(index),
            extra_mounts: vec![(MOUNT, Arc::new(fs.clone()))],
        },
        record,
    );
    // execute_response records the Runner's actual outcome before parsing CGI.
    // A successful PHP exit alone must never accept an incomplete RPC frame.
    record["php_completed"] = json!(record["ok"] == true);
    record["ok"] = json!(false);
    let finished = fs.finish(); // Always seal, including failed CGI capture/parse.
    record["rpc_valid"] = json!(finished.is_ok());
    if let Err(error) = &finished {
        record["rpc_error"] = json!(format!("{error:#}").chars().take(4096).collect::<String>());
    }
    record["rpc"] = rpc_contract::snapshot_value(&fs.snapshot()?);
    let body = execution.context("fixed CGI private RPC response")?;
    let prefix = if index == 2 { b"".as_slice() } else { LITERAL };
    let json_body = body
        .strip_prefix(prefix)
        .context("literal RPC-like CGI body prefix missing")?;
    record["stdout_literal_hex"] = json!(hex(prefix));
    record["php"] =
        serde_json::from_slice::<Value>(json_body).context("CGI private RPC JSON suffix")?;
    record["ok"] = json!(finished.is_ok());
    Ok(())
}

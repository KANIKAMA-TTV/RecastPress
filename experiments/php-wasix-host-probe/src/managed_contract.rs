//! Closed three-patch candidate; keeps the original native expectations.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const RECEIPT: &str = "experiments/php-wasix-managed-access-probe/evidence/build.json";
const MODULE: &str = "target/php-wasix-managed-access-probe/php-fiber-cgi.wasm";
const VARIANT: &str = "nts-fresh-wasi-zend-fibers-managed-access-v1";
const SOURCES: [(&str, &[u8]); 14] = [
    (
        "experiments/php-wasix-managed-access-probe/manifest.json",
        include_bytes!("../../php-wasix-managed-access-probe/manifest.json"),
    ),
    (
        "experiments/php-wasix-managed-access-probe/patch-plan.json",
        include_bytes!("../../php-wasix-managed-access-probe/patch-plan.json"),
    ),
    (
        "experiments/php-wasix-managed-access-probe/patches/0001-wasi-managed-access.patch",
        include_bytes!(
            "../../php-wasix-managed-access-probe/patches/0001-wasi-managed-access.patch"
        ),
    ),
    (
        "experiments/php-wasix-managed-access-probe/fixtures/recastpress_fs_access.h",
        include_bytes!("../../php-wasix-managed-access-probe/fixtures/recastpress_fs_access.h"),
    ),
    (
        "experiments/php-wasix-fiber-cgi-probe/manifest.json",
        include_bytes!("../../php-wasix-fiber-cgi-probe/manifest.json"),
    ),
    (
        "experiments/php-wasix-fiber-cgi-probe/fixtures/fiber_cgi_contract.php",
        include_bytes!("../../php-wasix-fiber-cgi-probe/fixtures/fiber_cgi_contract.php"),
    ),
    (
        "experiments/php-wasix-fiber-cgi-probe/evidence/native-cgi-fiber-contract.json",
        include_bytes!("../../php-wasix-fiber-cgi-probe/evidence/native-cgi-fiber-contract.json"),
    ),
    (
        "experiments/php-wasix-fiber-probe/manifest.json",
        include_bytes!("../../php-wasix-fiber-probe/manifest.json"),
    ),
    (
        "experiments/php-wasix-fiber-probe/patches/0001-nts-fresh-wasi-zend-fibers.patch",
        include_bytes!("../../php-wasix-fiber-probe/patches/0001-nts-fresh-wasi-zend-fibers.patch"),
    ),
    (
        "experiments/php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S",
        include_bytes!("../../php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S"),
    ),
    (
        "experiments/php-wasix-fiber-probe/fixtures/fiber_contract.php",
        include_bytes!("../../php-wasix-fiber-probe/fixtures/fiber_contract.php"),
    ),
    (
        "experiments/php-wasix-cgi-build/manifest.json",
        include_bytes!("../../php-wasix-cgi-build/manifest.json"),
    ),
    (
        "experiments/php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch",
        include_bytes!("../../php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch"),
    ),
    (
        "experiments/php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json",
        include_bytes!(
            "../../php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json"
        ),
    ),
];
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read_fixed(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = root.join(name);
    ensure!(path.canonicalize()? == path, "managed input symlink");
    let file = File::open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() > 0 && meta.len() <= limit,
        "managed input size/type"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == meta.len(),
        "managed input changed while reading"
    );
    Ok(bytes)
}
fn profile(report: &mut Value) {
    report["variant"] = json!(VARIANT);
    report["artifact"] = json!({"path":MODULE,"verified":false,"identity_policy":"same-job-three-source-patch-build"});
    report["build_receipt"] = json!({"path":RECEIPT,"verified":false,"full_source_tool_chain_verified_by_host":false,
        "chain_owner":"managed_check.py verifies all source/tool/patch/module provenance"});
    report["product_abi_selected"] = json!(false);
    report["source"] = json!({"repository":"wasix-org/php",
        "commit":"f4bfae79d5c19028349082f707930cf0951d23b9",
        "kind":"same-job-patched-source-build"});
}
pub fn describe_cgi(report: &mut Value) {
    super::cgi_contract::describe(report);
    profile(report);
    report["schema"] = json!("recastpress.php-wasix-managed-cgi-host.v0.5");
    report["scope"] = json!("three-patch-managed-candidate-fixed-CGI-request-regression-only");
    report["build_receipt"]["full_source_tool_config_chain_verified_by_host"] = json!(false);
    report["product_runtime_selected"] = json!(false);
    report["managed_access_verified_for_fixed_cases"] = json!(false);
}
pub fn verify_cgi(report: &mut Value) -> Result<()> {
    super::cgi_contract::verify_candidate(report, load_cgi_module)
}
fn load_cgi_module(report: &mut Value) -> Result<Vec<u8>> {
    let bytes = load_module(report)?;
    report["phase"] = json!("fiber_adapter_verification");
    report["postopt_adapter"] = super::fiber_adapter::inspect(&bytes)?;
    Ok(bytes)
}
pub fn describe_fiber(report: &mut Value) {
    super::fiber_cgi_contract::describe(report);
    profile(report);
    report["schema"] = json!("recastpress.php-wasix-managed-fiber-host.v0.5");
    report["scope"] = json!("three-patch-managed-candidate-fixed-Fiber-regression-only");
}
pub fn describe_runtime(report: &mut Value) {
    super::cgi_runtime_contract::describe(report);
    profile(report);
    report["schema"] = json!("recastpress.php-wasix-managed-runtime-host.v0.5");
    report["scope"] =
        json!("three-patch-managed-candidate-native-runtime-and-pure-permission-query-comparison");
    report["managed_access_verified_for_fixed_cases"] = json!(false);
    report["isolation"]["guest_mounts"] = json!(["/fixture", "/wp-core", "/scratch", "/rp-p0-fs"]);
    report["isolation"]["private_access_endpoint"] = json!("/rp-p0-fs/access");
    report["isolation"]["access_max_path_bytes"] = json!(4096);
    report["isolation"]["access_max_requests"] = json!(64);
}
pub fn verify_fiber(report: &mut Value) -> Result<()> {
    super::fiber_cgi_contract::verify_candidate(report, load_module)
}
pub fn verify_runtime(report: &mut Value) -> Result<()> {
    super::cgi_runtime_contract::verify_candidate(report, load_module, true)
}
fn load_module(report: &mut Value) -> Result<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let wire = read_fixed(&root, RECEIPT, 2 * 1024 * 1024)?;
    let receipt: Value = serde_json::from_slice(&wire)?;
    ensure!(
        receipt["schema"] == "recastpress.php-wasix-managed-access-build.v0.5"
            && receipt["variant"] == VARIANT
            && receipt["phase"] == "build_complete"
            && receipt["outcome"] == "managed_access_candidate_built_not_executed"
            && receipt["guest_executed"] == false,
        "managed source build did not complete"
    );
    report["source_files"] = json!([]);
    for (name, expected) in SOURCES {
        let bytes = read_fixed(&root, name, 256 * 1024)?;
        ensure!(
            bytes == expected,
            "managed source/oracle changed after compilation"
        );
        if !name.contains("/evidence/native-") {
            ensure!(
                receipt["input_files"][name] == sha(&bytes),
                "managed build input binding"
            );
        }
        report["source_files"]
            .as_array_mut()
            .context("managed sources")?
            .push(json!({"path":name,"bytes":bytes.len(),"sha256":sha(&bytes),"verified":true}));
    }
    let patches = receipt["source_patches"]
        .as_array()
        .context("three source patches")?;
    ensure!(
        patches.len() == 3 && patches.iter().all(|p| p["applied"] == true),
        "managed patch chain incomplete"
    );
    let bytes = read_fixed(&root, MODULE, 128 * 1024 * 1024)?;
    ensure!(
        receipt["php_wasm"]["path"] == MODULE
            && receipt["php_wasm"]["wasm_header_verified"] == true
            && receipt["php_wasm"]["executed"] == false
            && bytes.starts_with(b"\0asm\x01\0\0\0")
            && receipt["php_wasm"]["bytes"].as_u64() == Some(bytes.len() as u64)
            && receipt["php_wasm"]["sha256"] == sha(&bytes),
        "managed module identity"
    );
    report["artifact"]["observed_bytes"] = json!(bytes.len());
    report["artifact"]["observed_sha256"] = json!(sha(&bytes));
    report["artifact"]["verified"] = json!(true);
    report["source"]["verified"] = json!(true);
    report["build_receipt"]["observed_sha256"] = json!(sha(&wire));
    report["build_receipt"]["manifest_sha256"] =
        receipt["input_files"]["experiments/php-wasix-managed-access-probe/manifest.json"].clone();
    report["build_receipt"]["verified"] = json!(true);
    Ok(bytes)
}

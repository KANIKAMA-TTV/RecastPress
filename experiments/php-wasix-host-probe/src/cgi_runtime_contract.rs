//! Same combined candidate, fixed CGI language/hooks/IO corpus and fresh mounts.
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::Module;
use wasmer_wasix::runtime::module_cache::HashedModuleData;

use super::runtime_fixture_fs::{ORDINARY, READONLY, ScratchFs, ScratchSnapshot, wp_source_fs};

const BASE: &str = "experiments/php-wasix-cgi-runtime-probe";
const FIXTURE: &[u8] =
    include_bytes!("../../php-wasix-cgi-runtime-probe/fixtures/runtime_cgi_contract.php");
const MANIFEST: &[u8] = include_bytes!("../../php-wasix-cgi-runtime-probe/manifest.json");
const NATIVE: &[u8] =
    include_bytes!("../../php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json");
const SOURCES: [(&str, &[u8]); 5] = [
    (
        "experiments/php-wasix-cgi-runtime-probe/manifest.json",
        MANIFEST,
    ),
    (
        "experiments/php-wasix-cgi-runtime-probe/fixtures/runtime_cgi_contract.php",
        FIXTURE,
    ),
    (
        "experiments/php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json",
        NATIVE,
    ),
    (
        "experiments/php-wasi-host-probe/wordpress-fixture.json",
        include_bytes!("../../php-wasi-host-probe/wordpress-fixture.json"),
    ),
    (
        "experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php",
        include_bytes!("../../php-runtime-candidate-probe/fixtures/runtime_contract.php"),
    ),
];
const WP_NAMES: [&str; 3] = [
    "plugin.php",
    "class-wp-hook.php",
    "class-wp-filter-sentinel.php",
];

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn read_fixed(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = root.join(name);
    ensure!(
        path.canonicalize()? == path,
        "runtime source resolves through a symlink"
    );
    let file = File::open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() > 0 && meta.len() <= limit,
        "runtime source size/type"
    );
    let mut wire = Vec::new();
    file.take(limit + 1).read_to_end(&mut wire)?;
    ensure!(
        wire.len() as u64 == meta.len(),
        "runtime source changed while reading"
    );
    Ok(wire)
}

pub fn describe(report: &mut Value) {
    super::fiber_cgi_contract::describe(report);
    report["schema"] = json!("recastpress.php-wasix-cgi-runtime-host.v0.5");
    report["scope"] = json!("same-job-combined-CGI-six-fixed-language-hooks-and-IO-cases");
    report["runtime_contract_verified_for_fixed_cases"] = json!(false);
    report["wordpress_pure_hooks_executed"] = json!(false);
    report["full_wordpress_bootstrap_executed"] = json!(false);
    report["php_metadata_matches_actual_readonly_denial"] = json!(false);
    report["runtime_source_files"] = json!([]);
    report["wordpress_source_files"] = json!({});
    report["isolation"]["guest_environment_supplied"] = json!([]);
    report["isolation"]["guest_mounts"] = json!(["/fixture", "/wp-core", "/scratch"]);
    report["isolation"]["writable_fixture_entries"] = json!(["/scratch/ordinary.txt"]);
    report["isolation"]["scratch_file_limit_bytes"] = json!(4096);
    report["limitations"] = json!([
        "Only the fixed six-case corpus and three pure WordPress hook files are exercised; no full WordPress bootstrap, DB or product cache.",
        "Readonly PHP metadata is compared to native output separately from actual VFS mutation denial; disagreement remains a failure.",
        "Only ordinary.txt can change in the two-entry scratch mount. Other virtual-root writes are not globally forbidden.",
        "Fresh mounts and EOF do not prove complete task cancellation, process RSS bounds or aggregate resource reclamation.",
        "The external owned-child deadline/reap remains required. No product request CPU or in-process deadline is established."
    ]);
}

fn snapshot(value: &ScratchSnapshot) -> Value {
    json!({"ordinary_hex":hex(&value.ordinary), "readonly_hex":hex(&value.readonly),
        "ordinary_write_calls":value.ordinary_write_calls, "ordinary_bytes_written":value.ordinary_bytes_written,
        "ordinary_truncate_opens":value.ordinary_truncate_opens, "ordinary_set_len_calls":value.ordinary_set_len_calls,
        "readonly_mutating_open_rejections":value.readonly_mutating_open_rejections,
        "readonly_handle_mutation_rejections":value.readonly_handle_mutation_rejections,
        "namespace_mutation_rejections":value.namespace_mutation_rejections,
        "open_handles":value.open_handles, "closed_handles":value.closed_handles})
}

pub fn verify(report: &mut Value) -> Result<()> {
    report["phase"] = json!("cgi_runtime_build_verification");
    let bytes = super::fiber_cgi_contract::load_module(report)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    for (name, expected) in SOURCES {
        let wire = read_fixed(&root, name, 256 * 1024)?;
        ensure!(
            wire == expected,
            "runtime fixture/oracle changed since host compilation"
        );
        report["runtime_source_files"]
            .as_array_mut()
            .context("runtime sources")?
            .push(json!({"path":name,"bytes":wire.len(),"sha256":sha(&wire),"verified":true}));
    }
    let plan: Value = serde_json::from_slice(MANIFEST)?;
    let native: Value = serde_json::from_slice(NATIVE)?;
    ensure!(
        native["outcome"] == "native_cgi_runtime_contract_passed"
            && native["native_contract_passed"] == true,
        "native runtime oracle did not pass"
    );
    let expected = &native["observation"];
    ensure!(
        expected["cases"]
            .as_object()
            .is_some_and(|value| value.len() == 6),
        "six-case runtime oracle"
    );
    let environment: Vec<(String, String)> =
        serde_json::from_value(plan["guest_environment"].clone())?;
    report["isolation"]["guest_environment_supplied"] = json!(environment);
    report["native_oracle"] = json!({"path":format!("{BASE}/evidence/native-cgi-runtime-contract.json"),
        "sha256":sha(NATIVE),"expected_case_count":6,"expected_values_weakened":false});
    let mut sources = Vec::new();
    for name in WP_NAMES {
        let path = format!("target/php-wasix-cgi-runtime-probe/wp-core/{name}");
        let wire = read_fixed(&root, &path, 256 * 1024)?;
        ensure!(
            plan["wordpress"]["files"][name]["bytes"].as_u64() == Some(wire.len() as u64)
                && plan["wordpress"]["files"][name]["sha256"] == sha(&wire),
            "WP pure hook source identity"
        );
        report["wordpress_source_files"][name] =
            json!({"path":path,"bytes":wire.len(),"sha256":sha(&wire),"verified":true});
        sources.push((name, Arc::<[u8]>::from(wire)));
    }
    let source_fs = wp_source_fs(
        sources
            .try_into()
            .map_err(|_| anyhow::anyhow!("exact WP source count"))?,
    )?;
    report["phase"] = json!("fiber_adapter_verification");
    report["postopt_adapter"] = super::fiber_adapter::inspect(&bytes)?;
    let data = HashedModuleData::new(bytes);
    let engine = super::make_engine();
    report["engine_supports_async"] = json!(engine.supports_async());
    ensure!(
        engine.supports_async(),
        "CGI runtime corpus requires async-capable Fiber engine"
    );
    report["phase"] = json!("module_compile");
    eprintln!("PHP_CGI_RUNTIME_PHASE=module_compile");
    report["module_compile_invocations"] = json!(1);
    let started = Instant::now();
    let compiled = Module::new(&engine, &data.wasm()[..]);
    report["compile_elapsed_ms"] = json!(started.elapsed().as_millis());
    let module = compiled.context("compile same combined CGI candidate once")?;
    report["phase"] = json!("cgi_runtime_execution");
    let mut passed = true;
    for (index, name) in ["normal_first", "normal_repeat", "normal_final"]
        .into_iter()
        .enumerate()
    {
        let scratch = ScratchFs::new();
        let initial = scratch.snapshot()?;
        ensure!(
            initial.ordinary.is_empty() && initial.readonly == READONLY,
            "fresh scratch initial values"
        );
        let mut case = json!({"name":name,"request_number":index+1,"module_delivery":"Module.clone",
            "module_compile_invocations":0,"fresh_wasi_runtime":true,"fresh_scratch":true,"ok":false,
            "native_semantics_match":false,"guest_json":null,"guest_json_decoded":false,
            "guest_arguments":["-n","-d","fiber.stack_size=2097152","-d","register_argc_argv=0"],
            "program_sha256":sha(FIXTURE),"scratch_initial":snapshot(&initial)});
        let result = super::cgi_execution::execute(
            &engine,
            module.clone(),
            &data,
            super::cgi_execution::FixedRequest {
                filename: "runtime_cgi_contract.php",
                source: FIXTURE,
                environment: environment.clone(),
                extra_mounts: vec![
                    ("/wp-core", Arc::new(source_fs.clone())),
                    ("/scratch", Arc::new(scratch.clone())),
                ],
            },
            &mut case,
        );
        if case["guest_json"]["cases"]["wordpress_hooks"]["status"] == "ok" {
            report["wordpress_pure_hooks_executed"] = json!(true);
            report["wordpress_executed"] = json!(true);
        }
        if case["guest_json"]["cases"]["exceptions"]["status"] == "ok" {
            report["exceptions_executed"] = json!(true);
            report["exception_support"]["execution_verdict"] =
                json!("fixed-cgi-runtime-corpus-observed");
        }
        let final_state = scratch.snapshot()?;
        case["scratch_final"] = snapshot(&final_state);
        let result = result.and_then(|()| {
            ensure!(
                case["guest_json"] == *expected && case["response"] == native["response"],
                "CGI runtime corpus or response differs from native oracle"
            );
            ensure!(
                final_state.ordinary == ORDINARY
                    && final_state.readonly == READONLY
                    && final_state.ordinary_write_calls >= 1
                    && final_state.ordinary_bytes_written == ORDINARY.len()
                    && final_state.readonly_mutating_open_rejections >= 1
                    && final_state.open_handles == 0,
                "actual scratch IO, write denial or fixture handles did not match"
            );
            case["native_semantics_match"] = json!(true);
            Ok(())
        });
        case["expectation_met"] = json!(result.is_ok());
        if let Err(error) = result {
            passed = false;
            case["expectation_error"] =
                json!(format!("{error:#}").chars().take(4096).collect::<String>());
        }
        report["cases"]
            .as_array_mut()
            .context("runtime cases")?
            .push(case);
    }
    ensure!(passed, "CGI runtime native six-case comparison failed");
    report["runtime_contract_verified_for_fixed_cases"] = json!(true);
    report["php_metadata_matches_actual_readonly_denial"] = json!(true);
    report["wordpress_pure_hooks_executed"] = json!(true);
    report["wordpress_executed"] = json!(true);
    report["exceptions_executed"] = json!(true);
    report["exception_support"]["execution_verdict"] = json!("fixed-cgi-runtime-corpus-passed");
    report["integer_width_matches_native"] = json!(true);
    Ok(())
}

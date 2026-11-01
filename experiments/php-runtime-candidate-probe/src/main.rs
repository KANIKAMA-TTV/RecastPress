//! Inspect one pinned upstream artifact; parsing is not execution or validation.
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, path::Path};
use wasmparser::{Encoding, ExternalKind, Operator, Parser, Payload, TypeRef, WasmFeatures};

fn inspect() -> Result<serde_json::Value> {
    ensure!(
        std::env::args_os().len() == 1,
        "fixed artifact inspector accepts no arguments"
    );
    let identity: serde_json::Value = serde_json::from_str(include_str!("../artifact.json"))?;
    let size = identity["wasm_bytes"]
        .as_u64()
        .context("missing pinned length")?;
    ensure!(
        size <= 128 * 1024 * 1024,
        "artifact exceeds inspection budget"
    );
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.reference/php-wasix-8.5.12/php.wasm");
    let file = std::fs::File::open(path).context("prepare the pinned Actions artifact first")?;
    ensure!(file.metadata()?.len() == size, "wrong artifact length");
    let mut bytes = Vec::new();
    file.take(size + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 == size, "artifact changed while reading");
    let digest = format!("{:x}", Sha256::digest(&bytes));
    ensure!(identity["wasm_sha256"] == digest, "wrong artifact SHA-256");
    let mut imports: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut memories = Vec::new();
    let mut functions = 0_u64;
    let mut tags = 0_u64;
    let mut operators = 0_u64;
    let mut exceptions: BTreeMap<&str, u64> = BTreeMap::new();
    let mut start_export = false;
    let mut memory_export = false;
    let mut export_count = 0_u64;
    let mut export_sample = Vec::new();
    let mut parser = Parser::new(0);
    // Understand all encodings for observation. This does not enable any engine.
    parser.set_features(WasmFeatures::all());
    for payload in parser.parse_all(&bytes) {
        match payload? {
            Payload::Version { num, encoding, .. } => ensure!(
                num == 1 && encoding == Encoding::Module,
                "not a core Wasm module"
            ),
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import?;
                    if let TypeRef::Memory(memory) = import.ty {
                        memories.push(serde_json::json!({"import_module":import.module,"import_name":import.name,"initial_pages":memory.initial,"maximum_pages":memory.maximum,"shared":memory.shared,"memory64":memory.memory64}));
                    }
                    imports
                        .entry(import.module.to_owned())
                        .or_default()
                        .push(import.name.to_owned());
                }
            }
            Payload::MemorySection(section) => {
                for memory in section {
                    let memory = memory?;
                    memories.push(serde_json::json!({"initial_pages":memory.initial,"maximum_pages":memory.maximum,"shared":memory.shared,"memory64":memory.memory64}));
                }
            }
            Payload::TagSection(section) => tags += section.count() as u64,
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export?;
                    export_count += 1;
                    if export_sample.len() < 64 {
                        export_sample.push(serde_json::json!({"name":export.name,"kind":format!("{:?}",export.kind)}));
                    }
                    start_export |= export.name == "_start" && export.kind == ExternalKind::Func;
                    memory_export |= export.name == "memory" && export.kind == ExternalKind::Memory;
                }
            }
            Payload::CodeSectionEntry(body) => {
                functions += 1;
                let mut reader = body.get_operators_reader()?;
                while !reader.eof() {
                    operators += 1;
                    let name = match reader.read()? {
                        Operator::Try { .. } => Some("legacy_try"),
                        Operator::Catch { .. } => Some("legacy_catch"),
                        Operator::CatchAll => Some("legacy_catch_all"),
                        Operator::Rethrow { .. } => Some("legacy_rethrow"),
                        Operator::Delegate { .. } => Some("legacy_delegate"),
                        Operator::TryTable { .. } => Some("try_table"),
                        Operator::ThrowRef => Some("throw_ref"),
                        Operator::Throw { .. } => Some("throw"),
                        _ => None,
                    };
                    if let Some(name) = name {
                        *exceptions.entry(name).or_default() += 1;
                    }
                }
            }
            _ => {}
        }
    }
    for names in imports.values_mut() {
        names.sort();
    }
    let legacy = exceptions
        .iter()
        .filter(|(name, _)| name.starts_with("legacy_"))
        .map(|(_, count)| *count)
        .sum::<u64>();
    let wasix = imports.keys().any(|name| name.starts_with("wasix_"));
    let formal = exceptions.get("try_table").copied().unwrap_or(0)
        + exceptions.get("throw_ref").copied().unwrap_or(0);
    let foreign_imports = imports.keys().any(|name| name != "wasi_snapshot_preview1");
    Ok(serde_json::json!({
        "format_version":1,"process_pid":std::process::id(),"profile":"php_8_5_12_wasix_actions_artifact",
        "artifact_sha256":digest,"artifact_bytes":size,"declared_php_version":identity["declared_php_version"],
        "source_commit":identity["source_commit"],"workflow_run_id":identity["workflow_run_id"],
        "imports":imports,"memories":memories,"defined_functions":functions,"operators":operators,
        "exception_tags":tags,"exception_operators":exceptions,"legacy_exception_operators":legacy,
        "wasix_imports":wasix,"start_export":start_export,"memory_export":memory_export,
        "export_count":export_count,"export_sample":export_sample,
        "existing_wasip1_host_drop_in":!foreign_imports && legacy == 0 && formal == 0,
        "php_executed":false,"engine_validated":false,"product_ready":false
    }))
}
fn main() {
    match inspect() {
        Ok(value) => println!("PHP_RUNTIME_ARTIFACT_PROFILE={value}"),
        Err(error) => {
            eprintln!("PHP runtime artifact inspection failed: {error:#}");
            std::process::exit(1);
        }
    }
}

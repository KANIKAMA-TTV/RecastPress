//! Closed final-Wasm structural guard for the experimental Zend WASIX adapter.
//! This parses bytes only; it neither compiles nor executes a guest and does not
//! establish PHP/Fiber compatibility or native cancellation/resource safety.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmer::wasmparser::{
    CompositeInnerType, ExternalKind, GlobalType, Operator, Parser, Payload, TypeRef, ValType,
};

struct Global {
    ty: GlobalType,
    constant_i32: Option<i32>,
    imported: bool,
}

pub(crate) fn inspect(bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.starts_with(b"\0asm\x01\0\0\0"),
        "fiber adapter requires a core Wasm v1 module"
    );
    let mut types = Vec::new();
    let mut function_types = Vec::new();
    let mut imports = Vec::new();
    let mut globals = Vec::new();
    let mut memories = Vec::new();
    let mut exports = BTreeMap::new();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::TypeSection(reader) => {
                // Preserve every subtype index, including non-function members
                // of recursive groups. EH tag types and later function indices
                // must not be renumbered by a function-only filter.
                for group in reader {
                    for subtype in group?.into_types() {
                        let composite = subtype.composite_type;
                        types.push(match composite.inner {
                            CompositeInnerType::Func(ty) if !composite.shared => Some(ty),
                            _ => None,
                        });
                    }
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import?;
                    match import.ty {
                        TypeRef::Func(ty) | TypeRef::FuncExact(ty) => {
                            function_types.push(ty);
                            imports.push((import.module, import.name));
                        }
                        TypeRef::Global(ty) => globals.push(Global {
                            ty,
                            constant_i32: None,
                            imported: true,
                        }),
                        TypeRef::Memory(ty) => memories.push(ty),
                        _ => {}
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    function_types.push(ty?);
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global?;
                    let mut init = global.init_expr.get_operators_reader();
                    let constant_i32 = match init.read()? {
                        Operator::I32Const { value } => {
                            if matches!(init.read()?, Operator::End) && init.eof() {
                                Some(value)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    globals.push(Global {
                        ty: global.ty,
                        constant_i32,
                        imported: false,
                    });
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    memories.push(memory?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export?;
                    ensure!(
                        exports
                            .insert(export.name, (export.kind, export.index))
                            .is_none(),
                        "duplicate fiber adapter export"
                    );
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            Payload::UnknownSection { .. } => bail!("unknown core Wasm section"),
            _ => {}
        }
    }
    ensure!(
        bodies.len() + imports.len() == function_types.len(),
        "fiber function/code count mismatch"
    );
    let function_type = |index: u32| -> Result<_> {
        let ty = *function_types
            .get(index as usize)
            .context("fiber function index missing")?;
        types
            .get(ty as usize)
            .and_then(Option::as_ref)
            .context("fiber function type missing or unsupported")
    };
    let exported_function = |name: &str, params: &[ValType], results: &[ValType]| -> Result<u32> {
        let &(kind, index) = exports
            .get(name)
            .context("missing fiber adapter function export")?;
        ensure!(
            kind == ExternalKind::Func && index as usize >= imports.len(),
            "fiber export is not a defined function"
        );
        ensure!(
            (index as usize - imports.len()) < bodies.len(),
            "fiber function body missing"
        );
        let ty = function_type(index)?;
        ensure!(
            ty.params() == params && ty.results() == results,
            "fiber function signature mismatch"
        );
        Ok(index)
    };
    let switch = exported_function("rp_zend_wasix_switch_raw", &[ValType::I64], &[ValType::I32])?;
    let entry = exported_function("rp_zend_wasix_entry", &[], &[])?;
    let dispatch = exported_function("rp_zend_wasix_dispatch", &[], &[])?;
    ensure!(
        [switch, entry, dispatch]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len()
            == 3,
        "fiber exported functions alias"
    );

    let exported_global = |name: &str| -> Result<(u32, &Global)> {
        let &(kind, index) = exports
            .get(name)
            .context("missing fiber adapter global export")?;
        ensure!(
            kind == ExternalKind::Global,
            "fiber global export has wrong kind"
        );
        Ok((
            index,
            globals
                .get(index as usize)
                .context("fiber global index missing")?,
        ))
    };
    let (sp, stack_global) = exported_global("__stack_pointer")?;
    ensure!(
        stack_global.ty.mutable
            && !stack_global.ty.shared
            && stack_global.ty.content_type == ValType::I32,
        "actual SP must be nonshared mutable i32"
    );
    let (metadata_global, metadata) = exported_global("rp_zend_wasix_bootstrap_sp")?;
    ensure!(
        metadata_global != sp
            && !metadata.imported
            && !metadata.ty.mutable
            && !metadata.ty.shared
            && metadata.ty.content_type == ValType::I32,
        "bootstrap export must be a distinct defined immutable i32 address"
    );
    let address = u64::from(
        metadata
            .constant_i32
            .context("bootstrap address is not exactly i32.const/end")? as u32,
    );
    ensure!(
        address > 0 && address % 4 == 0,
        "bootstrap address is not a positive aligned wasm32 word"
    );
    let memory = memories
        .first()
        .context("bootstrap linear memory missing")?;
    ensure!(
        !memory.memory64 && memory.page_size_log2() == 16 && memory.initial <= 65536,
        "bootstrap requires ordinary wasm32 memory pages"
    );
    let initial_memory_bytes = memory
        .initial
        .checked_mul(65536)
        .context("initial memory size overflow")?;
    ensure!(
        address
            .checked_add(4)
            .is_some_and(|end| end <= initial_memory_bytes),
        "bootstrap word outside initial memory"
    );

    let read_body = |index: u32| -> Result<_> {
        let defined = (index as usize)
            .checked_sub(imports.len())
            .context("fiber shim is imported")?;
        let body = bodies.get(defined).context("fiber shim body missing")?;
        let locals = body
            .get_locals_reader()?
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            locals.len() <= 8
                && locals
                    .iter()
                    .map(|(count, _)| u64::from(*count))
                    .sum::<u64>()
                    <= 8,
            "unexpected fiber shim locals"
        );
        let mut reader = body.get_operators_reader()?;
        let mut ops = Vec::new();
        while !reader.eof() {
            ensure!(ops.len() < 32, "fiber shim exceeds closed operator bound");
            ops.push(reader.read()?);
        }
        Ok((locals, ops))
    };
    let (locals, ops) = read_body(switch)?;
    let (raw, switch_sequence, local_count) = match ops.as_slice() {
        [
            Operator::GlobalGet { global_index: get },
            Operator::LocalSet { local_index: 1 },
            Operator::LocalGet { local_index: 0 },
            Operator::Call {
                function_index: raw,
            },
            Operator::LocalGet { local_index: 1 },
            Operator::GlobalSet { global_index: set },
            Operator::End,
        ] if *get == sp && *set == sp && locals == [(1, ValType::I32)] => (
            *raw,
            vec![
                "global.get",
                "local.set",
                "local.get",
                "call",
                "local.get",
                "global.set",
                "end",
            ],
            1,
        ),
        [
            Operator::GlobalGet { global_index: get },
            Operator::LocalSet { local_index: 1 },
            Operator::LocalGet { local_index: 0 },
            Operator::Call {
                function_index: raw,
            },
            Operator::LocalSet { local_index: 2 },
            Operator::LocalGet { local_index: 1 },
            Operator::GlobalSet { global_index: set },
            Operator::LocalGet { local_index: 2 },
            Operator::End,
        ] if *get == sp
            && *set == sp
            && (locals == [(2, ValType::I32)]
                || locals == [(1, ValType::I32), (1, ValType::I32)]) =>
        {
            (
                *raw,
                vec![
                    "global.get",
                    "local.set",
                    "local.get",
                    "call",
                    "local.set",
                    "local.get",
                    "global.set",
                    "local.get",
                    "end",
                ],
                2,
            )
        }
        _ => bail!(
            "fiber switch must save SP in an i32 local, call raw import and immediately restore SP without stack memory operations"
        ),
    };
    ensure!(
        imports.get(raw as usize) == Some(&("wasix_32v1", "context_switch")),
        "fiber switch calls wrong import"
    );
    let raw_type = function_type(raw)?;
    ensure!(
        raw_type.params() == [ValType::I64] && raw_type.results() == [ValType::I32],
        "raw context_switch ABI mismatch"
    );

    let (entry_locals, entry_ops) = read_body(entry)?;
    ensure!(entry_locals.is_empty(), "fiber entry has locals/prologue");
    let effective_address = match entry_ops.as_slice() {
        [
            Operator::I32Const { value },
            Operator::I32Load { memarg },
            Operator::GlobalSet { global_index },
            Operator::Call { function_index },
            Operator::Unreachable,
            Operator::End,
        ] if memarg.memory == 0
            && memarg.align <= 2
            && *global_index == sp
            && *function_index == dispatch =>
        {
            u64::from(*value as u32)
                .checked_add(memarg.offset)
                .context("entry effective address overflow")?
        }
        _ => bail!(
            "fiber entry must load bound bootstrap word, install actual SP, call C dispatcher and trap on return"
        ),
    };
    ensure!(
        effective_address <= u64::from(u32::MAX) && effective_address == address,
        "fiber entry metadata address does not match exact exported symbol"
    );
    Ok(json!({
        "verified":true,
        "parser":"wasmer::wasmparser",
        "inspected_bytes":bytes.len(),
        "inspected_bytes_sha256":format!("{:x}", Sha256::digest(bytes)),
        "scope":"closed final-Wasm adapter shapes only; not PHP/Fiber, cancellation or resource correctness",
        "function_indices":{"rp_zend_wasix_switch_raw":switch,"rp_zend_wasix_entry":entry,"rp_zend_wasix_dispatch":dispatch},
        "stack_global_export":"__stack_pointer",
        "stack_global_index":sp,
        "stack_global_imported":stack_global.imported,
        "stack_global_type":"nonshared mutable i32",
        "bootstrap_export":"rp_zend_wasix_bootstrap_sp",
        "bootstrap_global_index":metadata_global,
        "bootstrap_export_type":"defined immutable i32 address",
        "bootstrap_address":address,
        "entry_effective_load_address":effective_address,
        "bootstrap_word_bytes":4,
        "initial_memory_bytes":initial_memory_bytes,
        "memory_count":memories.len(),
        "switch_import":{"module":"wasix_32v1","name":"context_switch","index":raw,"parameters":["i64"],"results":["i32"]},
        "switch_operator_sequence":switch_sequence,
        "switch_wasm_local_count":local_count,
        "entry_operator_sequence":["i32.const","i32.load","global.set","call","unreachable","end"],
        "entry_wasm_local_count":0,
        "linear_stack_memory_writes_in_shims":0,
        "target_stack_pointer_changed_before_switch":false,
        "restores_before_return_to_C":true,
        "sets_stack_before_initial_C_dispatch":true,
        "engine_invocations":0,
        "guest_executed":false,
        "php_fiber_compatibility_verified":false
    }))
}

#[cfg(test)]
mod tests {
    use super::inspect;

    /// Parser-only, including type/global index offsets and signed address bits.
    /// No Engine, Module, Store, instance or guest execution is involved.
    #[test]
    fn closed_shapes_bind_exact_symbols_and_reject_unsafe_mutants() {
        let template = r#"
        (module
          (rec (type $gc (struct (field i32))) (type $unused_type (func)))
          (import "unrelated" "global" (global $external i32))
          (import "unrelated" "function" (func $unused))
          (import "wasix_32v1" "context_switch" (func $raw (param i64) (result i32)))
          (memory 1)
          (global $sp (export "__stack_pointer") (mut i32) (i32.const 65536))
          (global $metadata (export "rp_zend_wasix_bootstrap_sp") i32 (i32.const 16))
          (global $fake (mut i32) (i32.const 32768))
          (tag $exception)
          (func $eh (block $caught (try_table (catch_all $caught) nop)))
          (func $extra)
          (func $lookalike (param i64) (result i32) i32.const 0)
          (func $dispatch (export "rp_zend_wasix_dispatch"))
          (func $entry (export "rp_zend_wasix_entry")
            i32.const 12 i32.load offset=4 align=4 global.set $sp
            call $dispatch unreachable)
          (func $switch (export "rp_zend_wasix_switch_raw") (param i64) (result i32)
            SWITCH_BODY))
        "#;
        let variants = [
            (
                1,
                "(local i32) global.get $sp local.set 1 local.get 0 call $raw local.get 1 global.set $sp",
            ),
            (
                2,
                "(local i32 i32) global.get $sp local.set 1 local.get 0 call $raw local.set 2 local.get 1 global.set $sp local.get 2",
            ),
        ];
        let mut rejected = 0;
        for (local_count, body) in variants {
            let source = template.replace("SWITCH_BODY", body);
            let accepted = inspect(&wat::parse_str(&source).unwrap()).unwrap();
            assert_eq!(accepted["verified"], true);
            assert_eq!(accepted["switch_wasm_local_count"], local_count);
            assert_eq!(accepted["stack_global_index"], 1);
            assert_eq!(accepted["bootstrap_global_index"], 2);
            assert_eq!(accepted["bootstrap_address"], 16);
            assert_eq!(accepted["entry_effective_load_address"], 16);
            assert_eq!(accepted["switch_import"]["index"], 1);
            let boundary = source
                .replace("(i32.const 16)", "(i32.const 65532)")
                .replace("i32.const 12 i32.load", "i32.const 65528 i32.load");
            assert!(inspect(&wat::parse_str(&boundary).unwrap()).is_ok());
            let high_address = source
                .replace("(memory 1)", "(memory 65536)")
                .replace("(i32.const 16)", "(i32.const -2147483648)")
                .replace("i32.const 12 i32.load", "i32.const 2147483644 i32.load");
            assert_eq!(
                inspect(&wat::parse_str(&high_address).unwrap()).unwrap()["bootstrap_address"],
                2147483648u64
            );

            let mutants = vec![
                ("linear stack store", source.replace("local.set 1 local.get 0", "local.set 1 i32.const 16 i32.const 9 i32.store local.get 0")),
                ("extra call", source.replace("call $raw", "call $raw call $extra")),
                ("SP prechange", source.replace("local.set 1 local.get 0", "local.set 1 i32.const 1234 global.set $sp local.get 0")),
                ("wrong raw name", source.replace("\"context_switch\"", "\"context_create\"")),
                ("wrong raw module", source.replace("\"wasix_32v1\"", "\"wasi_snapshot_preview1\"")),
                ("wrong raw ABI", source.replace("(func $raw (param i64)", "(func $raw (param i32)")),
                ("wrong switch ABI", source.replace("(export \"rp_zend_wasix_switch_raw\") (param i64)", "(export \"rp_zend_wasix_switch_raw\") (param i32)")),
                ("defined raw lookalike", source.replace("call $raw", "call $lookalike")),
                ("fake SP", source.replace("global.get $sp", "global.get $fake").replace("global.set $sp", "global.set $fake")),
                ("immutable SP", source.replace("(export \"__stack_pointer\") (mut i32)", "(export \"__stack_pointer\") i32")),
                ("mutable metadata", source.replace("(export \"rp_zend_wasix_bootstrap_sp\") i32", "(export \"rp_zend_wasix_bootstrap_sp\") (mut i32)")),
                ("metadata is not constant", source.replace("(i32.const 16)", "(global.get $external)")),
                ("metadata extended expression", source.replace("(i32.const 16)", "(i32.add (i32.const 12) (i32.const 4))")),
                ("unbound address", source.replace("(i32.const 16)", "(i32.const 20)")),
                ("zero address", source.replace("(i32.const 16)", "(i32.const 0)")),
                ("unaligned address", source.replace("(i32.const 16)", "(i32.const 17)")),
                ("word beyond memory", source.replace("(i32.const 16)", "(i32.const 65536)")),
                ("wrong offset", source.replace("offset=4", "offset=8")),
                ("offset overflows wasm32", source.replace("offset=4", "offset=4294967295")),
                ("load alignment", source.replace("align=4", "align=8")),
                ("load from wrong memory", source.replace("i32.load offset=4", "i32.load 1 offset=4")),
                ("wrong dispatcher", source.replace("call $dispatch unreachable", "call $extra unreachable")),
                ("C before SP", source.replace("global.set $sp\n            call $dispatch", "call $dispatch\n            global.set $sp")),
                ("entry store", source.replace("i32.const 12 i32.load", "i32.const 24 i32.const 9 i32.store i32.const 12 i32.load")),
                ("entry extra nop", source.replace("call $dispatch unreachable", "call $dispatch nop unreachable")),
                ("entry return", source.replace("call $dispatch unreachable", "call $dispatch")),
                ("entry locals", source.replace("(func $entry (export \"rp_zend_wasix_entry\")", "(func $entry (export \"rp_zend_wasix_entry\") (local i32)")),
                ("wrong local type", source.replace("(local i32", "(local i64")),
                ("extra local", source.replace("(local i32)", "(local i32 i32)").replace("(local i32 i32)", "(local i32 i32 i32)")),
                ("SP saved in parameter", source.replace("local.set 1 local.get 0", "local.set 0 local.get 0")),
                ("SP restored from parameter", source.replace("local.get 1 global.set $sp", "local.get 0 global.set $sp")),
                ("missing memory", source.replace("(memory 1)", "")),
                ("memory64", source.replace("(memory 1)", "(memory i64 1)")),
                ("imported metadata", source.replace("(import \"unrelated\" \"global\"", "(import \"unrelated\" \"metadata\" (global $metadata i32))\n          (import \"unrelated\" \"global\"")
                    .replace("(global $metadata (export \"rp_zend_wasix_bootstrap_sp\") i32 (i32.const 16))", "(export \"rp_zend_wasix_bootstrap_sp\" (global $metadata))")),
                ("metadata aliases SP", source.replace("(global $metadata (export \"rp_zend_wasix_bootstrap_sp\") i32 (i32.const 16))", "(export \"rp_zend_wasix_bootstrap_sp\" (global $sp))")),
                ("defined exports alias", source.replace("(export \"rp_zend_wasix_entry\")", "").replace("(func $dispatch (export \"rp_zend_wasix_dispatch\"))", "(func $dispatch (export \"rp_zend_wasix_dispatch\") (export \"rp_zend_wasix_entry\"))")),
                ("entry is imported", source.replace("(export \"rp_zend_wasix_entry\")", "").replace("(memory 1)", "(export \"rp_zend_wasix_entry\" (func $unused)) (memory 1)")),
                ("switch is imported", source.replace("(export \"rp_zend_wasix_switch_raw\")", "").replace("(memory 1)", "(export \"rp_zend_wasix_switch_raw\" (func $raw)) (memory 1)")),
            ];
            for (reason, mutant) in mutants {
                assert_ne!(mutant, source, "mutation did not apply: {reason}");
                let bytes = wat::parse_str(&mutant)
                    .unwrap_or_else(|error| panic!("WAT mutant {reason}: {error}"));
                assert!(
                    inspect(&bytes).is_err(),
                    "accepted {local_count}-local mutant: {reason}"
                );
                rejected += 1;
            }
            if local_count == 2 {
                let alias = source
                    .replace("local.set 2", "local.set 1")
                    .replace("local.get 2", "local.get 1");
                assert!(inspect(&wat::parse_str(&alias).unwrap()).is_err());
                rejected += 1;
            }
            let mut truncated = wat::parse_str(&source).unwrap();
            truncated.pop();
            assert!(inspect(&truncated).is_err());
            rejected += 1;
        }
        println!(
            "WASIX_FIBER_ADAPTER_UNIT={}",
            serde_json::json!({
                "normal_switch_shapes":2,"unsafe_mutants_rejected":rejected,
                "boundary_and_high_addresses_verified":true,
                "type_and_imported_global_offsets_verified":true,
                "guest_executed":false
            })
        );
    }
}

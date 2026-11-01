//! Fixed public fixture only, with one explicit new output directory.
use recastpress_wasm_publication_probe::prepare_fixed_publication;
use std::{io::Write, process::ExitCode};
fn failed() -> ExitCode {
    let _ = std::io::stderr()
        .lock()
        .write_all(b"Wasm publication preparation failed\n");
    ExitCode::from(2)
}
fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let result = match (args.next(), args.next()) {
        (Some(directory), None) => prepare_fixed_publication(std::path::Path::new(&directory)),
        _ => return failed(),
    };
    let value = match result {
        Ok(value) => value,
        Err(_) => return failed(),
    };
    let summary = format!(
        "WASM_PUBLICATION_PROBE_RESULT={{\"format_version\":1,\"profile\":\"wasmtime_36_0_19_fixed_component_prepared_bundle\",\"process_pid\":{},\"body_bytes\":{},\"bundle_bytes\":{},\"receipt_bytes\":{},\"post_return_completed\":{},\"guest_memory_changed_after_copy\":{},\"unpublished_only\":true}}",
        std::process::id(),
        value.body_bytes,
        value.bundle_bytes,
        value.receipt_bytes,
        value.post_return_completed,
        value.guest_memory_changed_after_copy
    );
    let mut output = std::io::stdout().lock();
    if output
        .write_all(summary.as_bytes())
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.flush())
        .is_err()
    {
        return failed();
    }
    ExitCode::SUCCESS
}

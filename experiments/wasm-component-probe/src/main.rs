//! Fixed owned Component WAT fixtures only; no guest file input.
use recastpress_wasm_component_probe::verify_fixed_fixtures;
fn main() {
    if std::env::args_os().len() != 1 {
        eprintln!("Component fixture probe does not accept arguments");
        std::process::exit(2);
    }
    match verify_fixed_fixtures() {
        Ok(value) => println!(
            "WASM_COMPONENT_PROBE_RESULT={{\"format_version\":1,\"profile\":\"wasmtime_36_0_19_fixed_component_model_fixtures\",\"process_pid\":{},\"fixture_checks\":{},\"literal_output_bytes\":{},\"fuel_per_store\":{},\"unpublished_only\":true}}",
            std::process::id(),
            value.fixture_checks,
            value.literal_output_bytes,
            value.fuel_per_store
        ),
        Err(error) => {
            eprintln!("Component fixture probe failed: {error}");
            std::process::exit(1);
        }
    }
}

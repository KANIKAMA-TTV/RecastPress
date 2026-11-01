#![forbid(unsafe_code)]
fn main() {
    if std::env::args_os().len() != 1 {
        eprintln!("fixed selected Wasm consumer failed");
        std::process::exit(2);
    }
    match recastpress_selected_wasm_consumer_probe::observe_current() {
        Ok(bytes) => println!(
            "SELECTED_WASM_CONSUMER_RESULT={{\"format_version\":1,\"process_pid\":{},\"body_bytes\":{},\"unpublished_only\":true}}",
            std::process::id(),
            bytes
        ),
        Err(_) => {
            eprintln!("fixed selected Wasm consumer failed");
            std::process::exit(1);
        }
    }
}

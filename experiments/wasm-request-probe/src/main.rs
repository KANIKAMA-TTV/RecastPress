//! Owned loopback experiment, never a production listener or arbitrary runner.
use std::{io::Write, process::ExitCode};
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        let _ = std::io::stderr()
            .lock()
            .write_all(b"Wasm request probe does not accept arguments\n");
        return ExitCode::from(2);
    }
    if recastpress_wasm_request_probe::run_loopback()
        .await
        .is_err()
    {
        let _ = std::io::stderr()
            .lock()
            .write_all(b"Wasm request probe failed\n");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

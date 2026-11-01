//! Register Unix shutdown handlers before announcing listener readiness.

use std::{future::Future, io};

#[cfg(unix)]
pub fn install() -> io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => eprintln!("RecastPress shutting down after SIGINT"),
            _ = terminate.recv() => eprintln!("RecastPress shutting down after SIGTERM"),
        }
    })
}

#[cfg(not(unix))]
pub fn install() -> io::Result<impl Future<Output = ()>> {
    Ok(async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => eprintln!("RecastPress shutting down after Ctrl+C"),
            Err(error) => eprintln!("RecastPress signal handler failed; shutting down: {error}"),
        }
    })
}

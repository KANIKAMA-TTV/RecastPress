//! One clock per engine; request deadlines belong to individual Store callbacks.
use anyhow::{Context, Result};
use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};
use wasmtime::Engine;

pub const TICK_MILLIS: u64 = 10;

pub struct EpochClock {
    cancel: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl EpochClock {
    pub fn new(engine: Engine) -> Result<Self> {
        let (cancel, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("rp-p0-epoch".into())
            .spawn(move || {
                while matches!(
                    receiver.recv_timeout(Duration::from_millis(TICK_MILLIS)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    engine.increment_epoch();
                }
            })
            .context("cannot start the shared epoch clock")?;
        Ok(Self {
            cancel,
            thread: Some(thread),
        })
    }
}

impl Drop for EpochClock {
    fn drop(&mut self) {
        let _ = self.cancel.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

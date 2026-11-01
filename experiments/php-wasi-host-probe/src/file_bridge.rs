//! Closed fixture RPC: PHP flushes a command; Rust writes a read-only guest reply.
//! This is a capability experiment, not a product ABI or a wpdb implementation.
use anyhow::{Context as _, Result, bail, ensure};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
};
use tokio::io::AsyncWrite;
use wasmtime_wasi::cli::{IsTerminal, StdoutStream};
use wasmtime_wasi_io::{
    poll::Pollable,
    streams::{OutputStream, StreamError},
};

pub const OUTPUT_LIMIT: usize = 64 * 1024;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct State {
    directory: Option<PathBuf>,
    pending: Vec<u8>,
    stdout: Vec<u8>,
    wire_bytes: usize,
    cache: Option<u64>,
    nonce: u64,
    calls: usize,
    failed: bool,
}
impl Drop for State {
    fn drop(&mut self) {
        // Fallback for setup errors. No recursive deletion or path supplied by PHP.
        if let Some(directory) = &self.directory {
            let _ = std::fs::remove_file(directory.join("reply.json"));
            let _ = std::fs::remove_dir(directory);
        }
    }
}
#[derive(Clone)]
pub struct Bridge(Arc<Mutex<State>>);
impl Bridge {
    pub fn new(nonce: u64) -> Result<Self> {
        let base =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/php-wasi-host-probe/rpc");
        std::fs::create_dir_all(&base)?;
        let directory = base.join(format!(
            "{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory)?;
        let state = State {
            directory: Some(directory),
            pending: Vec::new(),
            stdout: Vec::new(),
            wire_bytes: 0,
            cache: None,
            nonce,
            calls: 0,
            failed: false,
        };
        std::fs::write(
            state.directory.as_ref().unwrap().join("reply.json"),
            b"null\n",
        )?;
        Ok(Self(Arc::new(Mutex::new(state))))
    }
    pub fn directory(&self) -> PathBuf {
        self.0.lock().unwrap().directory.as_ref().unwrap().clone()
    }
    pub fn output(&self) -> Output {
        Output(self.clone())
    }
    pub fn captured(&self) -> Vec<u8> {
        let mut state = self.0.lock().unwrap();
        let pending = std::mem::take(&mut state.pending);
        if pending.starts_with(b"RP_P0 ") {
            state.failed = true;
        } else {
            state.stdout.extend(pending);
        }
        state.stdout.clone()
    }
    pub fn cleanup(&self) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        if let Some(directory) = &state.directory {
            std::fs::remove_file(directory.join("reply.json"))?;
            std::fs::remove_dir(directory)?;
        }
        state.directory = None;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        let state = self.0.lock().unwrap();
        ensure!(
            !state.failed && state.wire_bytes < OUTPUT_LIMIT,
            "stdout or RPC failure"
        );
        ensure!(
            state.pending.is_empty() && state.directory.is_none(),
            "incomplete RPC or cleanup"
        );
        Ok(())
    }
    pub fn check_calls(&self, expected: usize) -> Result<()> {
        ensure!(
            self.0.lock().unwrap().calls == expected,
            "missing live Rust callbacks"
        );
        Ok(())
    }
    fn write(&self, bytes: &[u8]) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        let result = Self::write_locked(&mut state, bytes);
        if result.is_err() {
            state.failed = true;
        }
        result
    }
    fn write_locked(state: &mut State, bytes: &[u8]) -> Result<()> {
        ensure!(
            bytes.len() <= OUTPUT_LIMIT.saturating_sub(state.wire_bytes),
            "stdout limit"
        );
        state.wire_bytes += bytes.len();
        state.pending.extend_from_slice(bytes);
        while let Some(end) = state.pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = state.pending.drain(..=end).collect();
            if !line.starts_with(b"RP_P0 ") {
                state.stdout.extend(line);
                continue;
            }
            ensure!(
                line.len() <= 128 && state.calls < 8,
                "RPC frame/count limit"
            );
            state.calls += 1;
            let reply = match line.as_slice() {
                b"RP_P0 CACHE_GET\n" => serde_json::json!({"value":state.cache}),
                b"RP_P0 CACHE_SET 7\n" => {
                    state.cache = Some(7);
                    serde_json::json!({"ok":true})
                }
                b"RP_P0 DB_SELECT_FIXTURE\n" => serde_json::json!({"rows":[{"nonce":state.nonce}]}),
                _ => bail!("unknown probe RPC"),
            };
            let reply = format!("{reply}\n");
            ensure!(reply.len() <= 1024, "RPC reply limit");
            let directory = state
                .directory
                .as_ref()
                .context("RPC after request cleanup")?;
            std::fs::write(directory.join("reply.json"), reply.as_bytes())?;
        }
        ensure!(
            !state.pending.starts_with(b"RP_P0 ") || state.pending.len() <= 128,
            "RPC frame limit"
        );
        Ok(())
    }
}
#[derive(Clone)]
pub struct Output(Bridge);
impl IsTerminal for Output {
    fn is_terminal(&self) -> bool {
        false
    }
}
impl StdoutStream for Output {
    fn async_stream(&self) -> Box<dyn AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
    fn p2_stream(&self) -> Box<dyn OutputStream> {
        Box::new(self.clone())
    }
}
#[async_trait::async_trait]
impl Pollable for Output {
    async fn ready(&mut self) {}
}
#[async_trait::async_trait]
impl OutputStream for Output {
    fn write(&mut self, bytes: Bytes) -> Result<(), StreamError> {
        self.0.write(&bytes).map_err(StreamError::Trap)
    }
    fn flush(&mut self) -> Result<(), StreamError> {
        Ok(())
    }
    fn check_write(&mut self) -> Result<usize, StreamError> {
        let mut state = self.0.0.lock().unwrap();
        let remaining = OUTPUT_LIMIT.saturating_sub(state.wire_bytes);
        if remaining == 0 {
            state.failed = true;
        }
        if state.failed {
            Err(StreamError::Closed)
        } else {
            Ok(remaining)
        }
    }
}
impl AsyncWrite for Output {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(
            self.0
                .write(bytes)
                .map(|()| bytes.len())
                .map_err(std::io::Error::other),
        )
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

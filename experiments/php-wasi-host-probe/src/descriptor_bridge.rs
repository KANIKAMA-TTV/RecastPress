//! Closed P0 protocol on a per-request file descriptor, independent of output.
use anyhow::{Result, bail, ensure};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub const OUTPUT_LIMIT: usize = 64 * 1024;
pub const REQUEST_LIMIT: usize = 128;
pub const CHANNEL_NAME: &[u8] = b"channel";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct State {
    directory: Option<PathBuf>,
    pending: Vec<u8>,
    total: usize,
    cache: Option<u64>,
    nonce: u64,
    calls: usize,
    failed: bool,
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(directory) = &self.directory {
            let _ = std::fs::remove_file(directory.join("channel"));
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
            total: 0,
            cache: None,
            nonce,
            calls: 0,
            failed: false,
        };
        std::fs::write(state.directory.as_ref().unwrap().join("channel"), b"null\n")?;
        Ok(Self(Arc::new(Mutex::new(state))))
    }
    pub fn directory(&self) -> PathBuf {
        self.0.lock().unwrap().directory.as_ref().unwrap().clone()
    }
    pub fn cleanup(&self) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        if let Some(directory) = &state.directory {
            std::fs::remove_file(directory.join("channel"))?;
            std::fs::remove_dir(directory)?;
        }
        state.directory = None;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        let state = self.0.lock().unwrap();
        ensure!(
            !state.failed && state.pending.is_empty() && state.directory.is_none(),
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
    pub fn receive(&self, bytes: &[u8]) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        let result = Self::receive_locked(&mut state, bytes);
        if result.is_err() {
            state.failed = true;
        }
        result
    }
    fn receive_locked(state: &mut State, bytes: &[u8]) -> Result<()> {
        ensure!(
            !state.failed && state.directory.is_some(),
            "RPC outside an active request"
        );
        ensure!(
            bytes.len() <= REQUEST_LIMIT && state.total + bytes.len() <= 1024,
            "RPC byte limit"
        );
        state.total += bytes.len();
        state.pending.extend_from_slice(bytes);
        while let Some(end) = state.pending.iter().position(|&byte| byte == b'\n') {
            ensure!(
                end < REQUEST_LIMIT && state.calls < 8,
                "RPC frame/count limit"
            );
            let line: Vec<u8> = state.pending.drain(..=end).collect();
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
            std::fs::write(
                state.directory.as_ref().unwrap().join("channel"),
                reply.as_bytes(),
            )?;
        }
        ensure!(state.pending.len() < REQUEST_LIMIT, "RPC frame limit");
        Ok(())
    }
}

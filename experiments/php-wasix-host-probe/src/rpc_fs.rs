//! Private, request-local P0 RPC file. No stdout parsing, host files or network.
//! The three commands match php-wasi-host-probe's closed descriptor fixture.
//! This is neither a product ABI, a real database nor a standard wpdb adapter.

use std::collections::BTreeMap;
use std::future::Future;
use std::io::{self, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use anyhow::{Result, ensure};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use wasmer_wasix::virtual_fs::{
    FileOpener, FileSystem, FileType, FsError, Metadata, OpenOptions, OpenOptionsConfig, ReadDir,
    VirtualFile,
};

pub const CHANNEL_NAME: &str = "channel";
pub const MAX_FRAME_BYTES: usize = 128;
pub const MAX_REQUEST_BYTES: usize = 1024;
pub const MAX_OPERATIONS: usize = 8;
pub const MAX_RESPONSE_BYTES: usize = 1024;
pub const MAX_REPLY_READ_BYTES: usize = MAX_OPERATIONS * MAX_RESPONSE_BYTES;

#[derive(Debug)]
struct State {
    nonce: u64,
    cache: BTreeMap<&'static str, u64>,
    pending: Vec<u8>,
    response: Vec<u8>,
    request_bytes: usize,
    operations: usize,
    response_bytes: usize,
    response_read_bytes: usize,
    open: bool,
    finished: bool,
    failure: Option<&'static str>,
}

impl State {
    fn fail(&mut self, reason: &'static str) -> io::Error {
        self.failure.get_or_insert(reason);
        io::Error::new(io::ErrorKind::InvalidData, reason)
    }

    fn active(&mut self) -> io::Result<()> {
        if let Some(reason) = self.failure {
            return Err(io::Error::new(io::ErrorKind::InvalidData, reason));
        }
        if self.finished || !self.open {
            return Err(self.fail("RPC outside an active channel"));
        }
        Ok(())
    }

    fn receive(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.active()?;
        if bytes.len() > MAX_FRAME_BYTES
            || bytes.len() > MAX_REQUEST_BYTES.saturating_sub(self.request_bytes)
        {
            return Err(self.fail("RPC write/request byte limit"));
        }
        self.request_bytes += bytes.len();
        for &byte in bytes {
            self.pending.push(byte);
            if byte == b'\n' {
                if self.operations == MAX_OPERATIONS {
                    return Err(self.fail("RPC operation limit"));
                }
                self.operations += 1;
                let reply = match self.pending.as_slice() {
                    b"RP_P0 CACHE_GET\n" => serde_json::json!({"value": self.cache.get("probe")}),
                    b"RP_P0 CACHE_SET 7\n" => {
                        self.cache.insert("probe", 7);
                        serde_json::json!({"ok": true})
                    }
                    b"RP_P0 DB_SELECT_FIXTURE\n" => {
                        serde_json::json!({"rows": [{"nonce": self.nonce}]})
                    }
                    _ => return Err(self.fail("unknown fixed RPC command")),
                };
                let response = format!("{reply}\n").into_bytes();
                if response.len() > MAX_RESPONSE_BYTES
                    || response.len()
                        > (MAX_OPERATIONS * MAX_RESPONSE_BYTES).saturating_sub(self.response_bytes)
                {
                    return Err(self.fail("RPC response byte limit"));
                }
                self.response_bytes += response.len();
                self.response = response;
                self.pending.clear();
            } else if self.pending.len() >= MAX_FRAME_BYTES {
                return Err(self.fail("RPC frame byte limit"));
            }
        }
        Ok(())
    }
}

/// Clones belong to the SAME request and share its single channel capability.
/// Create a new RpcFs for each request; no state is global or thread-local.
#[derive(Clone, Debug)]
pub struct RpcFs(Arc<Mutex<State>>);

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub operations: usize,
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub response_read_bytes: usize,
    pub pending_bytes: usize,
    pub cache_entries: usize,
    pub channel_open: bool,
    pub finished: bool,
    pub failure: Option<&'static str>,
    pub real_db: bool,
    pub standard_wpdb: bool,
}

impl RpcFs {
    pub fn new(nonce: u64) -> Self {
        Self(Arc::new(Mutex::new(State {
            nonce,
            cache: BTreeMap::new(),
            pending: Vec::new(),
            response: b"null\n".to_vec(),
            request_bytes: 0,
            operations: 0,
            response_bytes: 0,
            response_read_bytes: 0,
            open: false,
            finished: false,
            failure: None,
        })))
    }

    fn lock(&self) -> io::Result<MutexGuard<'_, State>> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("RPC state poisoned"))
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        let state = self.lock()?;
        Ok(Snapshot {
            operations: state.operations,
            request_bytes: state.request_bytes,
            response_bytes: state.response_bytes,
            response_read_bytes: state.response_read_bytes,
            pending_bytes: state.pending.len(),
            cache_entries: state.cache.len(),
            channel_open: state.open,
            finished: state.finished,
            failure: state.failure,
            real_db: false,
            standard_wpdb: false,
        })
    }

    /// Call only after dropping the guest's runner/runtime/handles. Seals this
    /// request even on failure; a swallowed guest IO error cannot become success.
    /// The integration must also check the expected operation count and output.
    pub fn finish(&self) -> Result<Snapshot> {
        {
            let mut state = self.lock()?;
            if state.open {
                state.fail("RPC channel still open at request completion");
            }
            if !state.pending.is_empty() {
                state.fail("incomplete RPC frame at request completion");
            }
            state.finished = true;
        }
        let snapshot = self.snapshot()?;
        ensure!(
            snapshot.failure.is_none(),
            "RPC failed: {:?}",
            snapshot.failure
        );
        Ok(snapshot)
    }

    fn deny(&self, reason: &'static str) -> FsError {
        if let Ok(mut state) = self.lock() {
            state.fail(reason);
        }
        FsError::PermissionDenied
    }
}

fn entry(path: &Path) -> std::result::Result<bool, FsError> {
    let mut name = None;
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) if name.is_none() => name = Some(part),
            _ => return Err(FsError::EntryNotFound),
        }
    }
    match name {
        None => Ok(false),
        Some(name) if name == CHANNEL_NAME => Ok(true),
        _ => Err(FsError::EntryNotFound),
    }
}

impl FileOpener for RpcFs {
    fn open(
        &self,
        path: &Path,
        options: &OpenOptionsConfig,
    ) -> std::result::Result<Box<dyn VirtualFile + Send + Sync>, FsError> {
        if !entry(path)? {
            return Err(FsError::NotAFile);
        }
        if !options.read
            || !options.write
            || options.append
            || options.truncate
            || options.create
            || options.create_new
        {
            return Err(self.deny("RPC requires existing read/write channel, no namespace changes"));
        }
        let mut state = self.lock().map_err(|_| FsError::IOError)?;
        if state.open || state.finished || state.failure.is_some() {
            state.fail("RPC channel already open, failed or finished");
            return Err(FsError::PermissionDenied);
        }
        state.open = true;
        Ok(Box::new(Channel {
            fs: self.clone(),
            position: 0,
            readback_ready: true,
            closed: false,
        }))
    }
}

impl FileSystem for RpcFs {
    fn readlink(&self, _: &Path) -> std::result::Result<PathBuf, FsError> {
        Err(FsError::InvalidInput)
    }
    fn read_dir(&self, _: &Path) -> std::result::Result<ReadDir, FsError> {
        // No directory listing, public directory or host path is supplied.
        Err(FsError::PermissionDenied)
    }
    fn metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        let file = entry(path)?;
        let len = if file {
            self.lock().map_err(|_| FsError::IOError)?.response.len() as u64
        } else {
            0
        };
        Ok(Metadata {
            ft: FileType {
                file,
                dir: !file,
                ..FileType::default()
            },
            accessed: 0,
            created: 0,
            modified: 0,
            len,
        })
    }
    fn symlink_metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        self.metadata(path)
    }
    fn new_open_options(&self) -> OpenOptions<'_> {
        OpenOptions::new(self)
    }
    fn create_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("RPC create_dir forbidden"))
    }
    fn create_symlink(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("RPC symlink forbidden"))
    }
    fn hard_link(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("RPC hard link forbidden"))
    }
    fn remove_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("RPC remove_dir forbidden"))
    }
    fn remove_file(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("RPC remove_file forbidden"))
    }
    fn rename<'a>(
        &'a self,
        _: &'a Path,
        _: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), FsError>> + Send + 'a>> {
        Box::pin(async { Err(self.deny("RPC rename forbidden")) })
    }
}

#[derive(Debug)]
struct Channel {
    fs: RpcFs,
    position: usize,
    readback_ready: bool,
    closed: bool,
}

impl Channel {
    fn check_open(&self) -> io::Result<()> {
        if self.closed {
            return Err(self
                .fs
                .lock()?
                .fail("RPC operation on closed channel handle"));
        }
        Ok(())
    }

    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let mut state = self.fs.lock()?;
        state.open = false;
        if !state.pending.is_empty() {
            return Err(state.fail("incomplete RPC frame on channel close"));
        }
        Ok(())
    }
}
impl Drop for Channel {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

impl AsyncRead for Channel {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.check_open()?;
        let mut state = this.fs.lock()?;
        state.active()?;
        // A zero-length stdio probe consumes nothing, including a partial frame.
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if !state.pending.is_empty() || !this.readback_ready {
            return Poll::Ready(Err(
                state.fail("RPC read before complete frame and seek zero")
            ));
        }
        let position = this.position.min(state.response.len());
        let count = (state.response.len() - position).min(buffer.remaining());
        let budget = MAX_REPLY_READ_BYTES.saturating_sub(state.response_read_bytes);
        if count > 0 && budget == 0 {
            return Poll::Ready(Err(state.fail("RPC reply read byte limit")));
        }
        let count = count.min(budget);
        buffer.put_slice(&state.response[position..position + count]);
        state.response_read_bytes += count;
        this.position += count;
        Poll::Ready(Ok(()))
    }
}
impl AsyncWrite for Channel {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.check_open()?;
        let mut state = this.fs.lock()?;
        state.active()?;
        if !bytes.is_empty() && state.pending.is_empty() && this.position != 0 {
            return Poll::Ready(Err(state.fail("RPC new command requires seek zero")));
        }
        state.receive(bytes)?;
        if !bytes.is_empty() {
            this.readback_ready = false;
        }
        this.position += bytes.len();
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        // A flush is allowed between partial writes, as in the old fixture.
        self.check_open()?;
        Poll::Ready(self.fs.lock()?.active())
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.get_mut().close())
    }
}
impl AsyncSeek for Channel {
    fn start_seek(self: Pin<&mut Self>, seek: SeekFrom) -> io::Result<()> {
        let this = self.get_mut();
        this.check_open()?;
        let mut state = this.fs.lock()?;
        state.active()?;
        match seek {
            // WASIX fd_write/fd_read synchronize the underlying file to the
            // descriptor cursor before EVERY IO, including fragmented writes.
            // Keeping the same position is not a protocol rewind or read grant.
            SeekFrom::Start(offset) if offset == this.position as u64 => Ok(()),
            SeekFrom::Start(0) if state.pending.is_empty() => {
                this.position = 0;
                this.readback_ready = true;
                Ok(())
            }
            SeekFrom::Current(0) => Ok(()),
            _ => Err(state.fail("RPC only supports seek zero after a complete frame")),
        }
    }
    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        self.check_open()?;
        self.fs.lock()?.active()?;
        Poll::Ready(Ok(self.position as u64))
    }
}
impl VirtualFile for Channel {
    fn last_accessed(&self) -> u64 {
        0
    }
    fn last_modified(&self) -> u64 {
        0
    }
    fn created_time(&self) -> u64 {
        0
    }
    fn size(&self) -> u64 {
        self.fs.lock().map(|s| s.response.len() as u64).unwrap_or(0)
    }
    fn is_open(&self) -> bool {
        !self.closed
    }
    fn set_times(&mut self, _: Option<u64>, _: Option<u64>) -> std::result::Result<(), FsError> {
        Err(self.fs.deny("RPC metadata mutation forbidden"))
    }
    fn set_len(&mut self, _: u64) -> std::result::Result<(), FsError> {
        Err(self.fs.deny("RPC truncate forbidden"))
    }
    fn unlink(&mut self) -> std::result::Result<(), FsError> {
        Err(self.fs.deny("RPC unlink forbidden"))
    }
    fn poll_read_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.check_open()?;
        let mut state = self.fs.lock()?;
        state.active()?;
        if !state.pending.is_empty() || !self.readback_ready {
            return Poll::Ready(Err(
                state.fail("RPC read readiness before complete frame and seek zero")
            ));
        }
        let remaining = state.response.len().saturating_sub(self.position);
        let budget = MAX_REPLY_READ_BYTES.saturating_sub(state.response_read_bytes);
        if remaining > 0 && budget == 0 {
            return Poll::Ready(Err(state.fail("RPC reply read byte limit")));
        }
        Poll::Ready(Ok(remaining.min(budget)))
    }
    fn poll_write_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.check_open()?;
        let mut state = self.fs.lock()?;
        state.active()?;
        Poll::Ready(Ok(
            MAX_FRAME_BYTES.min(MAX_REQUEST_BYTES.saturating_sub(state.request_bytes))
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }
    fn open(fs: &RpcFs) -> Box<dyn VirtualFile + Send + Sync> {
        fs.new_open_options()
            .read(true)
            .write(true)
            .open("/channel")
            .unwrap()
    }
    async fn call(file: &mut (dyn VirtualFile + Send + Sync), command: &[u8]) -> serde_json::Value {
        file.seek(SeekFrom::Start(0)).await.unwrap();
        file.write_all(command).await.unwrap();
        file.seek(SeekFrom::Start(0)).await.unwrap();
        let mut wire = Vec::new();
        file.read_to_end(&mut wire).await.unwrap();
        assert!(wire.ends_with(b"\n") && wire.len() <= MAX_RESPONSE_BYTES);
        serde_json::from_slice(&wire).unwrap()
    }

    #[test]
    fn fragmented_fixed_wire_seek_readback_and_reopen_use_live_rust_backend() {
        runtime().block_on(async {
            let fs = RpcFs::new(u64::MAX);
            let mut file = open(&fs);
            assert_eq!(
                call(&mut *file, b"RP_P0 CACHE_GET\n").await,
                serde_json::json!({"value":null})
            );
            file.seek(SeekFrom::Start(0)).await.unwrap();
            file.write_all(b"RP_P0 CACHE_").await.unwrap();
            file.flush().await.unwrap();
            file.write_all(b"SET 7\n").await.unwrap();
            file.seek(SeekFrom::Start(0)).await.unwrap();
            let mut wire = String::new();
            file.read_to_string(&mut wire).await.unwrap();
            assert_eq!(wire, "{\"ok\":true}\n");
            drop(file);
            let mut file = open(&fs.clone());
            assert_eq!(
                call(&mut *file, b"RP_P0 CACHE_GET\n").await,
                serde_json::json!({"value":7})
            );
            assert_eq!(
                call(&mut *file, b"RP_P0 DB_SELECT_FIXTURE\n").await,
                serde_json::json!({"rows":[{"nonce":u64::MAX}]})
            );
            drop(file);
            let snapshot = fs.finish().unwrap();
            assert_eq!(snapshot.operations, 4);
            assert_eq!(snapshot.cache_entries, 1);
            assert!(!snapshot.real_db && !snapshot.standard_wpdb && !snapshot.channel_open);
        });
    }

    #[test]
    fn separate_requests_have_independent_cache_nonce_failure_and_handle_state() {
        runtime().block_on(async {
            let a = RpcFs::new(11);
            let b = RpcFs::new(22);
            let mut fa = open(&a);
            let mut fb = open(&b);
            call(&mut *fa, b"RP_P0 CACHE_SET 7\n").await;
            assert_eq!(
                call(&mut *fb, b"RP_P0 CACHE_GET\n").await,
                serde_json::json!({"value":null})
            );
            assert_eq!(
                call(&mut *fa, b"RP_P0 DB_SELECT_FIXTURE\n").await,
                serde_json::json!({"rows":[{"nonce":11}]})
            );
            assert_eq!(
                call(&mut *fb, b"RP_P0 DB_SELECT_FIXTURE\n").await,
                serde_json::json!({"rows":[{"nonce":22}]})
            );
            fa.seek(SeekFrom::Start(0)).await.unwrap();
            assert!(fa.write_all(b"RP_P0 UNKNOWN\n").await.is_err());
            call(&mut *fb, b"RP_P0 CACHE_SET 7\n").await;
            drop((fa, fb));
            assert!(a.finish().is_err());
            assert!(b.finish().is_ok());
            assert!(a.clone().snapshot().unwrap().failure.is_some());
        });
    }

    #[test]
    fn unknown_oversized_and_incomplete_frames_remain_failed_after_guest_ignores_errors() {
        runtime().block_on(async {
            for wire in [
                b"RP_P0 UNKNOWN\n".to_vec(),
                vec![b'x'; MAX_FRAME_BYTES + 1],
                b"RP_P0 CACHE_".to_vec(),
            ] {
                let fs = RpcFs::new(1);
                let mut file = open(&fs);
                let _ = file.write_all(&wire).await;
                drop(file);
                assert!(fs.snapshot().unwrap().failure.is_some());
                assert!(fs.finish().is_err());
                assert!(
                    fs.new_open_options()
                        .read(true)
                        .write(true)
                        .open("channel")
                        .is_err()
                );
            }
            let fs = RpcFs::new(1);
            let mut file = open(&fs);
            file.write_all(&[b'x'; 64]).await.unwrap();
            assert!(file.write_all(&[b'x'; 64]).await.is_err());
            assert!(file.write_all(b"\n").await.is_err());
            drop(file);
            assert!(fs.finish().is_err());
        });
    }

    #[test]
    fn operation_budget_and_protocol_phase_errors_are_latched() {
        runtime().block_on(async {
            let fs = RpcFs::new(1);
            let mut file = open(&fs);
            for _ in 0..MAX_OPERATIONS {
                call(&mut *file, b"RP_P0 CACHE_GET\n").await;
            }
            file.seek(SeekFrom::Start(0)).await.unwrap();
            assert!(file.write_all(b"RP_P0 CACHE_GET\n").await.is_err());
            drop(file);
            assert_eq!(fs.snapshot().unwrap().operations, MAX_OPERATIONS);
            assert!(fs.finish().is_err());
            for phase in 0..3 {
                let fs = RpcFs::new(1);
                let mut file = open(&fs);
                file.write_all(b"RP_P0 CACHE_").await.unwrap();
                let failed = match phase {
                    0 => file.read(&mut [0; 8]).await.is_err(),
                    1 => file.seek(SeekFrom::Start(0)).await.is_err(),
                    _ => file.shutdown().await.is_err(),
                };
                assert!(failed);
                drop(file);
                assert!(fs.finish().is_err());
            }
        });
    }

    #[test]
    fn one_channel_capability_cannot_be_opened_concurrently_or_after_finish() {
        let fs = RpcFs::new(1);
        let file = open(&fs);
        assert!(
            fs.clone()
                .new_open_options()
                .read(true)
                .write(true)
                .open("channel")
                .is_err()
        );
        drop(file);
        assert!(fs.finish().is_err());
        let fs = RpcFs::new(1);
        let file = open(&fs);
        assert!(fs.finish().is_err());
        drop(file);
        assert!(fs.finish().is_err());
        let fs = RpcFs::new(1);
        drop(open(&fs));
        assert!(fs.finish().is_ok());
        assert!(
            fs.new_open_options()
                .read(true)
                .write(true)
                .open("channel")
                .is_err()
        );
        assert!(fs.finish().is_err());
    }

    #[test]
    fn closed_handle_cannot_access_a_reopened_channel_and_drop_cannot_close_it() {
        runtime().block_on(async {
            let fs = RpcFs::new(1);
            let mut old = open(&fs);
            call(&mut *old, b"RP_P0 CACHE_SET 7\n").await;
            old.shutdown().await.unwrap();
            let mut current = open(&fs.clone());
            drop(old);
            assert!(fs.snapshot().unwrap().channel_open);
            assert_eq!(
                call(&mut *current, b"RP_P0 CACHE_GET\n").await,
                serde_json::json!({"value":7})
            );
            drop(current);
            assert!(fs.finish().is_ok());

            let fs = RpcFs::new(1);
            let mut old = open(&fs);
            old.shutdown().await.unwrap();
            let current = open(&fs);
            assert!(old.write_all(b"RP_P0 CACHE_SET 7\n").await.is_err());
            assert_eq!(fs.snapshot().unwrap().operations, 0);
            drop((old, current));
            assert!(fs.finish().is_err());
        });
    }

    #[test]
    fn wasix_stdio_cursor_sync_and_zero_length_probes_preserve_fragmented_frames() {
        runtime().block_on(async {
            let fs = RpcFs::new(1);
            let mut file = open(&fs);
            let wire = b"RP_P0 CACHE_GET\n";
            file.write_all(&wire[..9]).await.unwrap();
            assert_eq!(file.read(&mut []).await.unwrap(), 0);
            assert_eq!(file.write(&[]).await.unwrap(), 0);
            assert_eq!(file.seek(SeekFrom::Start(9)).await.unwrap(), 9);
            file.write_all(&wire[9..]).await.unwrap();
            assert_eq!(
                file.seek(SeekFrom::Start(wire.len() as u64)).await.unwrap(),
                wire.len() as u64
            );
            file.seek(SeekFrom::Start(0)).await.unwrap();
            let mut first = [0; 3];
            file.read_exact(&mut first).await.unwrap();
            file.seek(SeekFrom::Start(3)).await.unwrap();
            let mut rest = Vec::new();
            file.read_to_end(&mut rest).await.unwrap();
            let response = [first.as_slice(), rest.as_slice()].concat();
            assert_eq!(response, b"{\"value\":null}\n");
            file.seek(SeekFrom::Start(response.len() as u64))
                .await
                .unwrap();
            assert_eq!(file.read(&mut [0]).await.unwrap(), 0);
            drop(file);
            let result = fs.finish().unwrap();
            assert_eq!(result.operations, 1);
            assert_eq!(result.request_bytes, wire.len());
            assert_eq!(result.response_bytes, response.len());
            assert_eq!(result.response_read_bytes, response.len());
        });
    }

    #[test]
    fn reply_rereads_share_a_request_budget_that_reopen_cannot_reset() {
        runtime().block_on(async {
            let fs = RpcFs::new(1);
            let mut file = open(&fs);
            file.write_all(b"RP_P0 CACHE_GET\n").await.unwrap();
            for _ in 0..MAX_REPLY_READ_BYTES {
                file.seek(SeekFrom::Start(0)).await.unwrap();
                let mut byte = [0];
                file.read_exact(&mut byte).await.unwrap();
                assert_eq!(byte, [b'{']);
            }
            assert_eq!(
                fs.snapshot().unwrap().response_read_bytes,
                MAX_REPLY_READ_BYTES
            );
            drop(file);
            // Closing/reopening does not replenish the shared request budget.
            let mut file = open(&fs.clone());
            assert!(file.read(&mut [0]).await.is_err());
            assert!(file.read(&mut [0]).await.is_err());
            drop(file);
            assert_eq!(
                fs.snapshot().unwrap().response_read_bytes,
                MAX_REPLY_READ_BYTES
            );
            assert_eq!(
                fs.snapshot().unwrap().failure,
                Some("RPC reply read byte limit")
            );
            assert!(fs.finish().is_err());
            assert!(
                fs.new_open_options()
                    .read(true)
                    .write(true)
                    .open("channel")
                    .is_err()
            );
        });
    }

    #[test]
    fn namespace_flags_paths_and_metadata_mutations_are_rejected() {
        runtime().block_on(async {
            for flag in 0..4 {
                let fs = RpcFs::new(1);
                assert!(
                    fs.new_open_options()
                        .read(true)
                        .write(true)
                        .append(flag == 0)
                        .truncate(flag == 1)
                        .create(flag == 2)
                        .create_new(flag == 3)
                        .open("channel")
                        .is_err()
                );
                assert!(fs.finish().is_err());
            }
            let fs = RpcFs::new(1);
            for path in ["other", "../channel", "nested/channel", "channel/child"] {
                assert!(
                    fs.new_open_options()
                        .read(true)
                        .write(true)
                        .open(path)
                        .is_err()
                );
            }
            assert!(fs.read_dir(Path::new("/")).is_err());
            assert!(fs.metadata(Path::new("channel")).unwrap().is_file());
            let path = Path::new("channel");
            assert_eq!(fs.create_dir(path), Err(FsError::PermissionDenied));
            assert_eq!(
                fs.create_symlink(path, path),
                Err(FsError::PermissionDenied)
            );
            assert_eq!(fs.hard_link(path, path), Err(FsError::PermissionDenied));
            assert_eq!(fs.remove_dir(path), Err(FsError::PermissionDenied));
            assert_eq!(fs.remove_file(path), Err(FsError::PermissionDenied));
            assert_eq!(fs.rename(path, path).await, Err(FsError::PermissionDenied));
            assert!(fs.finish().is_err());
            for mutation in 0..3 {
                let fs = RpcFs::new(1);
                let mut file = open(&fs);
                let result = match mutation {
                    0 => file.set_len(0),
                    1 => file.unlink(),
                    _ => file.set_times(Some(1), None),
                };
                assert_eq!(result, Err(FsError::PermissionDenied));
                drop(file);
                assert!(fs.finish().is_err());
            }
        });
    }
}

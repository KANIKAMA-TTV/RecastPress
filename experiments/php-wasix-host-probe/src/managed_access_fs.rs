//! Closed /rp-p0-fs/access P0 channel, not a host-FS or product authorization API.
//! Queries consult the same request's fixed mounts without opening target files.

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

use super::runtime_fixture_fs::{ScratchFs, WpSourceFs};

pub(crate) const CHANNEL_NAME: &str = "access";
pub(crate) const MAX_PATH_BYTES: usize = 4096;
pub(crate) const MAX_FRAME_BYTES: usize = 7 + MAX_PATH_BYTES;
pub(crate) const MAX_OPERATIONS: usize = 64;
pub(crate) const MAX_REQUEST_BYTES: usize = MAX_OPERATIONS * MAX_FRAME_BYTES;
pub(crate) const MAX_RESPONSE_BYTES: usize = MAX_OPERATIONS * 5;
// Includes byte-at-a-time request/reply IO, seeks, flushes and EOF probes.
const MAX_IO_CALLS: usize = MAX_REQUEST_BYTES + MAX_RESPONSE_BYTES * 2 + MAX_OPERATIONS * 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Status {
    Allow = 0,
    Denied = 1,
    Missing = 2,
    Unmanaged = 3,
    Invalid = 4,
    Io = 5,
}

#[derive(Clone, Debug)]
struct Policy {
    fixture_name: &'static str,
    wp: WpSourceFs,
    scratch: ScratchFs,
}

/// Absolute lexical paths only, with .. clamped to this virtual root.
/// No symlink lookup, host path interpretation, percent decoding or IO occurs.
fn normalized_path(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_PATH_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    if !text.starts_with('/') || text.contains(['\0', '\\', ':']) {
        return None;
    }
    let mut parts = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            name => parts.push(name),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

impl Policy {
    fn query(&self, bytes: &[u8], mode: u8) -> Status {
        let Some(path) = normalized_path(bytes) else {
            return Status::Invalid;
        };
        let (mount, entry) = path[1..].split_once('/').unwrap_or((&path[1..], ""));
        let result = match mount {
            "fixture" => {
                if entry.is_empty() {
                    Ok(mode & 2 == 0)
                } else if entry == self.fixture_name {
                    Ok(mode & 3 == 0)
                } else {
                    Err(FsError::EntryNotFound)
                }
            }
            "wp-core" => self.wp.access(Path::new(entry), mode),
            "scratch" => self.scratch.access(Path::new(entry), mode),
            _ => return Status::Unmanaged,
        };
        match result {
            Ok(true) => Status::Allow,
            Ok(false) | Err(FsError::PermissionDenied) => Status::Denied,
            Err(FsError::EntryNotFound | FsError::BaseNotDirectory | FsError::NotAFile) => {
                Status::Missing
            }
            Err(FsError::InvalidInput) => Status::Invalid,
            Err(_) => Status::Io,
        }
    }
}

#[derive(Debug)]
struct State {
    policy: Policy,
    pending: Vec<u8>,
    response: Option<[u8; 5]>,
    response_read: usize,
    requests: usize,
    allowed: usize,
    denied: usize,
    missing: usize,
    unmanaged: usize,
    invalid: usize,
    io_errors: usize,
    input_bytes: usize,
    generated_bytes: usize,
    read_bytes: usize,
    active_handles: usize,
    closed_handles: usize,
    open_events: usize,
    reused_transactions: usize,
    eof_reads: usize,
    io_calls: usize,
    finished: bool,
    protocol_error: Option<&'static str>,
}

impl State {
    fn fail(&mut self, reason: &'static str) -> io::Error {
        self.protocol_error.get_or_insert(reason);
        io::Error::new(io::ErrorKind::InvalidData, reason)
    }

    fn live(&mut self) -> io::Result<()> {
        if self.finished || self.active_handles != 1 {
            return Err(self.fail("access channel is not active"));
        }
        Ok(())
    }

    fn io_call(&mut self) -> io::Result<()> {
        self.live()?;
        if self.io_calls >= MAX_IO_CALLS {
            return Err(self.fail("access IO call limit"));
        }
        self.io_calls += 1;
        Ok(())
    }

    fn writable(&mut self) -> io::Result<()> {
        if let Some(reason) = self.protocol_error {
            return Err(io::Error::new(io::ErrorKind::InvalidData, reason));
        }
        Ok(())
    }

    fn receive(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writable()?;
        if bytes.is_empty() {
            return Ok(());
        }
        if self.response.is_some() {
            if self.response_read != 5 {
                return Err(self.fail("access next request before reply consumed"));
            }
            self.response = None;
            self.response_read = 0;
            self.reused_transactions += 1;
        }
        if self.requests == MAX_OPERATIONS {
            return Err(self.fail("access operation limit"));
        }
        if bytes.len() > MAX_FRAME_BYTES.saturating_sub(self.pending.len())
            || bytes.len() > MAX_REQUEST_BYTES.saturating_sub(self.input_bytes)
        {
            return Err(self.fail("access frame/request byte limit"));
        }
        self.input_bytes += bytes.len();
        self.pending.extend_from_slice(bytes);
        if self.pending.len() >= 4 && &self.pending[..4] != b"RPA1" {
            return Err(self.fail("access request magic"));
        }
        if self.pending.len() >= 5 && self.pending[4] > 7 {
            return Err(self.fail("access request mode"));
        }
        if self.pending.len() < 7 {
            return Ok(());
        }
        let length = usize::from(u16::from_le_bytes([self.pending[5], self.pending[6]]));
        if length == 0 || length > MAX_PATH_BYTES {
            return Err(self.fail("access path length"));
        }
        let expected = 7 + length;
        if self.pending.len() > expected {
            return Err(self.fail("access trailing bytes or concatenated frame"));
        }
        if self.pending.len() < expected {
            return Ok(());
        }
        let status = self.policy.query(&self.pending[7..], self.pending[4]);
        self.requests += 1;
        match status {
            Status::Allow => self.allowed += 1,
            Status::Denied => self.denied += 1,
            Status::Missing => self.missing += 1,
            Status::Unmanaged => self.unmanaged += 1,
            Status::Invalid => self.invalid += 1,
            Status::Io => self.io_errors += 1,
        }
        if self.generated_bytes > MAX_RESPONSE_BYTES - 5 {
            return Err(self.fail("access generated reply byte limit"));
        }
        self.generated_bytes += 5;
        self.response = Some([b'R', b'P', b'R', b'1', status as u8]);
        self.pending.clear();
        Ok(())
    }

    fn readable_bytes(&mut self, position: usize, readback_ready: bool) -> usize {
        if self.protocol_error.is_some() {
            return 0;
        }
        if !self.pending.is_empty() || self.response.is_none() || !readback_ready {
            self.fail("access read before complete request and seek zero");
            return 0;
        }
        if position != self.response_read {
            self.fail("access reply replay or cursor mismatch");
            return 0;
        }
        5 - self.response_read
    }
}

/// Clones share one request-local channel. Construct a new provider per request.
#[derive(Clone, Debug)]
pub(crate) struct ManagedAccessFs(Arc<Mutex<State>>);

#[derive(Clone, Debug, PartialEq, Eq)]
// Handle counters cover VFiles created here, not aliases in WASIX's fd/inode cache.
pub(crate) struct AccessSnapshot {
    pub requests: usize,
    pub allowed: usize,
    pub denied: usize,
    pub missing: usize,
    pub unmanaged: usize,
    pub invalid: usize,
    pub io_errors: usize,
    pub input_bytes: usize,
    pub generated_bytes: usize,
    pub read_bytes: usize,
    pub pending_bytes: usize,
    pub unread_response_bytes: usize,
    pub active_handles: usize,
    pub closed_handles: usize,
    pub open_events: usize,
    pub reused_transactions: usize,
    pub eof_reads: usize,
    pub io_calls: usize,
    pub finished: bool,
    pub protocol_error: Option<&'static str>,
}

impl AccessSnapshot {
    pub(crate) fn report_value(&self) -> serde_json::Value {
        serde_json::json!({"requests":self.requests,"allowed":self.allowed,"denied":self.denied,"missing":self.missing,"unmanaged":self.unmanaged,"invalid":self.invalid,"io_errors":self.io_errors,"input_bytes":self.input_bytes,"generated_bytes":self.generated_bytes,"read_bytes":self.read_bytes,"pending_bytes":self.pending_bytes,"unread_response_bytes":self.unread_response_bytes,"active_handles":self.active_handles,"closed_handles":self.closed_handles,"open_events":self.open_events,"reused_transactions":self.reused_transactions,"eof_reads":self.eof_reads,"io_calls":self.io_calls,"finished":self.finished,"protocol_error":self.protocol_error})
    }
}

impl ManagedAccessFs {
    pub(crate) fn new(
        fixture_name: &'static str,
        wp: WpSourceFs,
        scratch: ScratchFs,
    ) -> Result<Self> {
        ensure!(
            !fixture_name.is_empty()
                && fixture_name.len() <= 255
                && !fixture_name.contains(['/', '\\', ':', '\0'])
                && !matches!(fixture_name, "." | ".."),
            "fixed access fixture must be one virtual filename"
        );
        Ok(Self(Arc::new(Mutex::new(State {
            policy: Policy {
                fixture_name,
                wp,
                scratch,
            },
            pending: Vec::new(),
            response: None,
            response_read: 0,
            requests: 0,
            allowed: 0,
            denied: 0,
            missing: 0,
            unmanaged: 0,
            invalid: 0,
            io_errors: 0,
            input_bytes: 0,
            generated_bytes: 0,
            read_bytes: 0,
            active_handles: 0,
            closed_handles: 0,
            open_events: 0,
            reused_transactions: 0,
            eof_reads: 0,
            io_calls: 0,
            finished: false,
            protocol_error: None,
        }))))
    }

    fn lock(&self) -> io::Result<MutexGuard<'_, State>> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("access state poisoned"))
    }

    pub(crate) fn snapshot(&self) -> Result<AccessSnapshot> {
        let state = self.lock()?;
        Ok(AccessSnapshot {
            requests: state.requests,
            allowed: state.allowed,
            denied: state.denied,
            missing: state.missing,
            unmanaged: state.unmanaged,
            invalid: state.invalid,
            io_errors: state.io_errors,
            input_bytes: state.input_bytes,
            generated_bytes: state.generated_bytes,
            read_bytes: state.read_bytes,
            pending_bytes: state.pending.len(),
            unread_response_bytes: if state.response.is_some() {
                5 - state.response_read
            } else {
                0
            },
            active_handles: state.active_handles,
            closed_handles: state.closed_handles,
            open_events: state.open_events,
            reused_transactions: state.reused_transactions,
            eof_reads: state.eof_reads,
            io_calls: state.io_calls,
            finished: state.finished,
            protocol_error: state.protocol_error,
        })
    }

    /// Seal only after the runner/runtime/underlying VirtualFile has dropped.
    /// Zero requests are permitted here; the fixed host comparison must require
    /// its expected positive query count before claiming the provider was used.
    pub(crate) fn finish(&self) -> Result<AccessSnapshot> {
        {
            let mut state = self.lock()?;
            if state.active_handles != 0 {
                state.fail("access channel still active at completion");
            }
            if !state.pending.is_empty() {
                state.fail("access incomplete frame at completion");
            }
            if state.response.is_some() && state.response_read != 5 {
                state.fail("access unread reply at completion");
            }
            state.finished = true;
        }
        let snapshot = self.snapshot()?;
        ensure!(
            snapshot.protocol_error.is_none(),
            "access protocol failed: {:?}",
            snapshot.protocol_error
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

fn channel_entry(path: &Path) -> std::result::Result<bool, FsError> {
    let raw = path.to_str().ok_or(FsError::EntryNotFound)?;
    if raw.contains(['\\', ':', '\0']) {
        return Err(FsError::EntryNotFound);
    }
    let mut name = None;
    for part in path.components() {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) if name.is_none() => name = Some(value),
            _ => return Err(FsError::EntryNotFound),
        }
    }
    match name {
        None => Ok(false),
        Some(name) if name == CHANNEL_NAME => Ok(true),
        _ => Err(FsError::EntryNotFound),
    }
}

impl FileOpener for ManagedAccessFs {
    fn open(
        &self,
        path: &Path,
        options: &OpenOptionsConfig,
    ) -> std::result::Result<Box<dyn VirtualFile + Send + Sync>, FsError> {
        if !channel_entry(path)? {
            return Err(FsError::NotAFile);
        }
        if !options.read
            || !options.write
            || options.create
            || options.create_new
            || options.append
            || options.truncate
        {
            return Err(self.deny("access requires existing read/write channel"));
        }
        let mut state = self.lock().map_err(|_| FsError::IOError)?;
        if state.active_handles != 0
            || state.finished
            || state.protocol_error.is_some()
            || state.open_events == MAX_OPERATIONS
        {
            state.fail("access channel concurrent, failed, finished or open limit");
            return Err(FsError::PermissionDenied);
        }
        state.active_handles = 1;
        state.open_events += 1;
        Ok(Box::new(Channel {
            fs: self.clone(),
            position: 0,
            readback_ready: false,
            closed: false,
        }))
    }
}

impl FileSystem for ManagedAccessFs {
    fn readlink(&self, _: &Path) -> std::result::Result<PathBuf, FsError> {
        Err(FsError::InvalidInput)
    }
    fn read_dir(&self, _: &Path) -> std::result::Result<ReadDir, FsError> {
        Err(FsError::PermissionDenied)
    }
    fn metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        let file = channel_entry(path)?;
        let length = if file
            && self
                .lock()
                .map_err(|_| FsError::IOError)?
                .response
                .is_some()
        {
            5
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
            len: length,
        })
    }
    fn symlink_metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        self.metadata(path)
    }
    fn new_open_options(&self) -> OpenOptions<'_> {
        OpenOptions::new(self)
    }
    fn create_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("access create_dir forbidden"))
    }
    fn create_symlink(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("access symlink forbidden"))
    }
    fn hard_link(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("access hardlink forbidden"))
    }
    fn remove_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("access remove_dir forbidden"))
    }
    fn remove_file(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.deny("access remove_file forbidden"))
    }
    fn rename<'a>(
        &'a self,
        _: &'a Path,
        _: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), FsError>> + Send + 'a>> {
        Box::pin(async { Err(self.deny("access rename forbidden")) })
    }
}

#[derive(Debug)]
struct Channel {
    fs: ManagedAccessFs,
    position: usize,
    readback_ready: bool,
    closed: bool,
}

impl Channel {
    fn check_open(&self) -> io::Result<()> {
        if self.closed {
            return Err(self.fs.lock()?.fail("access use after handle close"));
        }
        Ok(())
    }
    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let mut state = self.fs.lock()?;
        state.active_handles = 0;
        state.closed_handles += 1;
        if !state.pending.is_empty() {
            return Err(state.fail("access incomplete frame on close"));
        }
        if state.response.is_some() && state.response_read != 5 {
            return Err(state.fail("access unread reply on close"));
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
        state.io_call()?;
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let remaining = state.readable_bytes(this.position, this.readback_ready);
        if remaining == 0 {
            state.eof_reads += 1;
            return Poll::Ready(Ok(()));
        }
        let count = remaining.min(buffer.remaining());
        if count > MAX_RESPONSE_BYTES.saturating_sub(state.read_bytes) {
            return Poll::Ready(Err(state.fail("access reply read byte limit")));
        }
        let reply = state.response.expect("readable_bytes requires a reply");
        buffer.put_slice(&reply[this.position..this.position + count]);
        this.position += count;
        state.response_read += count;
        state.read_bytes += count;
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
        state.io_call()?;
        let starting =
            state.pending.is_empty() && (state.response.is_none() || state.response_read == 5);
        state.receive(bytes)?;
        if !bytes.is_empty() {
            if starting {
                this.position = 0;
            }
            this.position += bytes.len();
            this.readback_ready = false;
        }
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.check_open()?;
        let mut state = self.fs.lock()?;
        state.io_call()?;
        Poll::Ready(state.writable())
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
        state.io_call()?;
        state.writable()?;
        match seek {
            // WASIX synchronizes the VFile cursor before every fragmented IO.
            SeekFrom::Start(offset) if offset == this.position as u64 => Ok(()),
            SeekFrom::Start(0) if state.pending.is_empty() => {
                this.position = 0;
                this.readback_ready = state.response.is_some();
                Ok(())
            }
            SeekFrom::Current(0) => Ok(()),
            _ => Err(state.fail("access unsupported seek")),
        }
    }
    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        self.check_open()?;
        let mut state = self.fs.lock()?;
        state.io_call()?;
        state.writable()?;
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
        self.fs
            .lock()
            .map(|s| if s.response.is_some() { 5 } else { 0 })
            .unwrap_or(0)
    }
    fn is_open(&self) -> bool {
        !self.closed
    }
    fn set_times(&mut self, _: Option<u64>, _: Option<u64>) -> std::result::Result<(), FsError> {
        Err(self.fs.deny("access metadata mutation forbidden"))
    }
    fn set_len(&mut self, _: u64) -> std::result::Result<(), FsError> {
        Err(self.fs.deny("access truncate forbidden"))
    }
    fn unlink(&mut self) -> std::result::Result<(), FsError> {
        Err(self.fs.deny("access unlink forbidden"))
    }
    fn poll_read_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.check_open()?;
        let mut state = self.fs.lock()?;
        state.io_call()?;
        Poll::Ready(Ok(state.readable_bytes(self.position, self.readback_ready)))
    }
    fn poll_write_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.check_open()?;
        let mut state = self.fs.lock()?;
        state.io_call()?;
        state.writable()?;
        if state.response.is_some() && state.response_read != 5 {
            return Poll::Ready(Err(
                state.fail("access write readiness before reply consumed")
            ));
        }
        if state.requests == MAX_OPERATIONS {
            return Poll::Ready(Err(state.fail("access operation limit")));
        }
        Poll::Ready(Ok(MAX_FRAME_BYTES
            .saturating_sub(state.pending.len())
            .min(MAX_REQUEST_BYTES.saturating_sub(state.input_bytes))))
    }
}

#[cfg(test)]
mod tests {
    use super::super::runtime_fixture_fs::wp_source_fs;
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }
    fn provider() -> (ManagedAccessFs, ScratchFs) {
        let wp = wp_source_fs(
            [
                "plugin.php",
                "class-wp-hook.php",
                "class-wp-filter-sentinel.php",
            ]
            .map(|name| (name, Arc::<[u8]>::from(b"<?php // test\n".as_slice()))),
        )
        .unwrap();
        let scratch = ScratchFs::new();
        (
            ManagedAccessFs::new("runtime_cgi_contract.php", wp, scratch.clone()).unwrap(),
            scratch,
        )
    }
    fn open(fs: &ManagedAccessFs) -> Box<dyn VirtualFile + Send + Sync> {
        fs.new_open_options()
            .read(true)
            .write(true)
            .open("/access")
            .unwrap()
    }
    fn frame(path: &[u8], mode: u8) -> Vec<u8> {
        let mut bytes = b"RPA1".to_vec();
        bytes.push(mode);
        bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        bytes.extend_from_slice(path);
        bytes
    }
    async fn reply(file: &mut (dyn VirtualFile + Send + Sync)) -> u8 {
        file.flush().await.unwrap();
        file.seek(SeekFrom::Start(0)).await.unwrap();
        let mut bytes = [0; 5];
        file.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes[..4], b"RPR1");
        let mut eof = [0; 1];
        assert_eq!(file.read(&mut eof).await.unwrap(), 0);
        bytes[4]
    }
    async fn query(file: &mut (dyn VirtualFile + Send + Sync), path: &[u8], mode: u8) -> u8 {
        let bytes = frame(path, mode);
        file.seek(SeekFrom::Start(0)).await.unwrap();
        file.write_all(&bytes[..7]).await.unwrap();
        file.write_all(&bytes[7..]).await.unwrap();
        reply(file).await
    }

    #[test]
    fn managed_policy_queries_match_fixed_files_without_mutating_scratch() {
        runtime().block_on(async {
            let (fs, scratch) = provider();
            let before = scratch.snapshot().unwrap();
            let mut file = open(&fs);
            for (path, mode, expected) in [
                ("/scratch/ordinary.txt", 6, 0),
                ("/scratch/readonly.txt", 4, 0),
                ("/scratch/readonly.txt", 2, 1),
                ("/scratch/ordinary.txt", 1, 1),
                ("/wp-core/plugin.php", 4, 0),
                ("/wp-core/plugin.php", 2, 1),
                ("/fixture/runtime_cgi_contract.php", 0, 0),
                ("/fixture/runtime_cgi_contract.php", 2, 1),
                ("/scratch", 5, 0),
                ("/scratch", 2, 1),
            ] {
                assert_eq!(query(&mut *file, path.as_bytes(), mode).await, expected);
            }
            drop(file);
            assert_eq!(before, scratch.snapshot().unwrap());
            let snapshot = fs.finish().unwrap();
            assert_eq!(
                (snapshot.requests, snapshot.allowed, snapshot.denied),
                (10, 5, 5)
            );
            assert_eq!((snapshot.generated_bytes, snapshot.read_bytes), (50, 50));
            assert_eq!((snapshot.active_handles, snapshot.closed_handles), (0, 1));
        });
    }

    #[test]
    fn lexical_paths_keep_managed_missing_and_unmanaged_results_distinct() {
        runtime().block_on(async {
            let (fs, _) = provider();
            let mut file = open(&fs);
            for (path, expected) in [
                ("/scratch/./ordinary.txt", 0),
                ("/wp-core/../scratch//ordinary.txt", 0),
                ("/../../scratch/ordinary.txt", 0),
                ("/scratch/missing", 2),
                ("/wp-core/nested/plugin.php", 2),
                ("/fixture/missing", 2),
                ("/scratch-other/ordinary.txt", 3),
                ("/etc/passwd", 3),
                ("relative", 4),
                ("/scratch/ord\0inary.txt", 4),
                ("/C:/private", 4),
            ] {
                assert_eq!(query(&mut *file, path.as_bytes(), 4).await, expected);
            }
            assert_eq!(query(&mut *file, b"/scratch/\xff", 4).await, 4);
            drop(file);
            let snapshot = fs.finish().unwrap();
            assert_eq!(
                (
                    snapshot.allowed,
                    snapshot.missing,
                    snapshot.unmanaged,
                    snapshot.invalid
                ),
                (3, 3, 2, 4)
            );
        });
    }

    #[test]
    fn fragmented_frame_and_cached_handle_reuse_preserve_reply_accounting() {
        runtime().block_on(async {
            let (fs, scratch) = provider();
            let mut file = open(&fs);
            let bytes = frame(b"/scratch/ordinary.txt", 2);
            for (position, byte) in bytes.iter().enumerate() {
                file.seek(SeekFrom::Start(position as u64)).await.unwrap();
                file.write_all(&[*byte]).await.unwrap();
            }
            assert_eq!(reply(&mut *file).await, 0);
            // No reopen: the same VFile may survive C fclose in WASIX's inode cache.
            assert_eq!(query(&mut *file, b"/scratch/readonly.txt", 2).await, 1);
            drop(file);
            let mut reopened = open(&fs.clone());
            assert_eq!(query(&mut *reopened, b"/wp-core/plugin.php", 4).await, 0);
            drop(reopened);
            let snapshot = fs.finish().unwrap();
            assert_eq!(
                (
                    snapshot.requests,
                    snapshot.open_events,
                    snapshot.closed_handles
                ),
                (3, 2, 2)
            );
            assert_eq!(snapshot.reused_transactions, 2);
            assert_eq!((snapshot.generated_bytes, snapshot.read_bytes), (15, 15));
            assert_eq!(scratch.snapshot().unwrap().open_handles, 0);
        });
    }

    #[test]
    fn malformed_headers_lengths_and_concatenated_frames_latch_failure() {
        runtime().block_on(async {
            let mut concatenated = frame(b"/scratch/ordinary.txt", 4);
            concatenated.extend_from_slice(&frame(b"/scratch/readonly.txt", 4));
            for bytes in [
                b"BAD!\x04\x01\x00/".to_vec(),
                b"RPA1\x08\x01\x00/".to_vec(),
                b"RPA1\x04\x00\x00".to_vec(),
                b"RPA1\x04\x01\x10".to_vec(),
                vec![b'x'; MAX_FRAME_BYTES + 1],
                concatenated,
            ] {
                let (fs, _) = provider();
                let mut file = open(&fs);
                assert!(file.write_all(&bytes).await.is_err());
                drop(file);
                assert!(fs.finish().is_err());
                assert!(fs.snapshot().unwrap().protocol_error.is_some());
            }
        });
    }

    #[test]
    fn incomplete_reads_unread_replies_and_replays_cannot_become_success() {
        runtime().block_on(async {
            let (fs, _) = provider();
            let mut file = open(&fs);
            file.write_all(b"RPA1\x04").await.unwrap();
            let mut one = [0; 1];
            assert_eq!(file.read(&mut one).await.unwrap(), 0); // immediate EOF, never pending
            drop(file);
            assert!(fs.finish().is_err());
            let (fs, _) = provider();
            let mut file = open(&fs);
            file.write_all(&frame(b"/scratch/ordinary.txt", 4))
                .await
                .unwrap();
            assert!(
                file.write_all(&frame(b"/scratch/readonly.txt", 4))
                    .await
                    .is_err()
            );
            drop(file);
            assert!(fs.finish().is_err());
            let (fs, _) = provider();
            let mut file = open(&fs);
            query(&mut *file, b"/scratch/ordinary.txt", 4).await;
            file.seek(SeekFrom::Start(0)).await.unwrap();
            assert_eq!(file.read(&mut one).await.unwrap(), 0);
            drop(file);
            assert!(fs.finish().is_err());
            let (fs, _) = provider();
            let mut file = open(&fs);
            file.write_all(b"RPA1").await.unwrap();
            drop(file);
            assert!(fs.finish().is_err());
        });
    }

    #[test]
    fn concurrent_channels_namespace_changes_and_operation_limits_are_bounded() {
        runtime().block_on(async {
            let (fs, _) = provider();
            let mut file = open(&fs);
            let longest_path = format!(
                "/scratch/{}",
                "x".repeat(MAX_PATH_BYTES - "/scratch/".len())
            );
            for _ in 0..MAX_OPERATIONS {
                assert_eq!(query(&mut *file, longest_path.as_bytes(), 4).await, 2);
            }
            assert!(
                file.write_all(&frame(b"/scratch/ordinary.txt", 4))
                    .await
                    .is_err()
            );
            drop(file);
            assert!(fs.finish().is_err());
            let snapshot = fs.snapshot().unwrap();
            assert_eq!(snapshot.requests, 64);
            assert_eq!(snapshot.input_bytes, MAX_REQUEST_BYTES);
            assert_eq!((snapshot.generated_bytes, snapshot.read_bytes), (320, 320));
            let (fs, _) = provider();
            let file = open(&fs);
            assert!(
                fs.new_open_options()
                    .read(true)
                    .write(true)
                    .open("access")
                    .is_err()
            );
            drop(file);
            assert!(fs.finish().is_err());
            for operation in 0..4 {
                let (fs, _) = provider();
                match operation {
                    0 => assert!(
                        fs.new_open_options()
                            .read(true)
                            .write(true)
                            .create(true)
                            .open("access")
                            .is_err()
                    ),
                    1 => assert!(fs.remove_file(Path::new("access")).is_err()),
                    2 => assert!(
                        fs.create_symlink(Path::new("access"), Path::new("alias"))
                            .is_err()
                    ),
                    _ => assert!(
                        fs.rename(Path::new("access"), Path::new("elsewhere"))
                            .await
                            .is_err()
                    ),
                }
                assert!(fs.finish().is_err());
            }
            let (unused, _) = provider();
            assert_eq!(unused.finish().unwrap().requests, 0); // Caller must not call this execution proof.
        });
    }
}

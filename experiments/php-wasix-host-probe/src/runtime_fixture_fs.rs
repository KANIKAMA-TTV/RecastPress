//! Two closed, in-memory P0 mounts: three verified WP files and request scratch.
//! No host paths, general source tree, product storage or permission emulation.
//! Metadata has no writable/mode field; PHP is_writable remains an observation.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io::{self, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{Result, ensure};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use wasmer_wasix::virtual_fs::{
    DirEntry, FileOpener, FileSystem, FileType, FsError, Metadata, OpenOptions, OpenOptionsConfig,
    ReadDir, VirtualFile,
};

pub(crate) const ORDINARY: &[u8] = b"ordinary\0\xff\r\n";
pub(crate) const READONLY: &[u8] = b"readonly fixture\n";
const MAX_SCRATCH_BYTES: usize = 4096;
const MAX_SOURCE_BYTES: usize = 256 * 1024;
const WP_NAMES: [&str; 3] = [
    "plugin.php",
    "class-wp-hook.php",
    "class-wp-filter-sentinel.php",
];

fn entry(path: &Path) -> std::result::Result<Option<&str>, FsError> {
    let raw = path.to_str().ok_or(FsError::EntryNotFound)?;
    if raw.contains(['\\', ':', '\0']) {
        return Err(FsError::EntryNotFound);
    }
    let mut name = None;
    for part in path.components() {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) if name.is_none() => {
                name = Some(value.to_str().ok_or(FsError::EntryNotFound)?);
            }
            _ => return Err(FsError::EntryNotFound),
        }
    }
    Ok(name)
}

fn metadata(file: bool, bytes: usize) -> Metadata {
    Metadata {
        ft: FileType {
            file,
            dir: !file,
            ..FileType::default()
        },
        accessed: 0,
        created: 0,
        modified: 0,
        len: bytes as u64,
    }
}

fn listing(names: impl IntoIterator<Item = (&'static str, usize)>) -> ReadDir {
    ReadDir::new(
        names
            .into_iter()
            .map(|(name, bytes)| DirEntry {
                path: PathBuf::from(format!("/{name}")),
                metadata: Ok(metadata(true, bytes)),
            })
            .collect(),
    )
}

/// Bytes must first pass the caller's pinned WP manifest/hash verification.
/// The FS accepts owned Arc bytes so verified files never need leaked storage.
#[derive(Clone, Debug)]
pub(crate) struct WpSourceFs {
    files: Arc<BTreeMap<&'static str, Arc<[u8]>>>,
}

pub(crate) fn wp_source_fs(files: [(&'static str, Arc<[u8]>); 3]) -> Result<WpSourceFs> {
    ensure!(
        files.iter().map(|(name, _)| *name).collect::<BTreeSet<_>>()
            == WP_NAMES.into_iter().collect(),
        "WP fixture must contain exactly the three fixed files"
    );
    ensure!(
        files
            .iter()
            .all(|(_, bytes)| !bytes.is_empty() && bytes.len() <= MAX_SOURCE_BYTES),
        "WP fixture file byte budget"
    );
    Ok(WpSourceFs {
        files: Arc::new(files.into_iter().collect()),
    })
}

impl FileOpener for WpSourceFs {
    fn open(
        &self,
        path: &Path,
        options: &OpenOptionsConfig,
    ) -> std::result::Result<Box<dyn VirtualFile + Send + Sync>, FsError> {
        if options.would_mutate() || !options.read {
            return Err(FsError::PermissionDenied);
        }
        let name = entry(path)?.ok_or(FsError::NotAFile)?;
        let bytes = self.files.get(name).ok_or(FsError::EntryNotFound)?;
        Ok(Box::new(MemoryHandle::new(
            Backing::Source(bytes.clone()),
            true,
            false,
        )))
    }
}

impl FileSystem for WpSourceFs {
    fn readlink(&self, _: &Path) -> std::result::Result<PathBuf, FsError> {
        Err(FsError::InvalidInput)
    }
    fn read_dir(&self, path: &Path) -> std::result::Result<ReadDir, FsError> {
        if let Some(name) = entry(path)? {
            return Err(if self.files.contains_key(name) {
                FsError::BaseNotDirectory
            } else {
                FsError::EntryNotFound
            });
        }
        Ok(listing(
            self.files.iter().map(|(name, bytes)| (*name, bytes.len())),
        ))
    }
    fn metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        match entry(path)? {
            None => Ok(metadata(false, 0)),
            Some(name) => self
                .files
                .get(name)
                .map(|bytes| metadata(true, bytes.len()))
                .ok_or(FsError::EntryNotFound),
        }
    }
    fn symlink_metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        self.metadata(path)
    }
    fn new_open_options(&self) -> OpenOptions<'_> {
        OpenOptions::new(self)
    }
    fn create_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn create_symlink(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn hard_link(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn remove_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn remove_file(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn rename<'a>(
        &'a self,
        _: &'a Path,
        _: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), FsError>> + Send + 'a>> {
        Box::pin(async { Err(FsError::PermissionDenied) })
    }
}

#[derive(Debug, Default)]
struct ScratchState {
    ordinary: Vec<u8>,
    ordinary_write_calls: usize,
    ordinary_bytes_written: usize,
    ordinary_truncate_opens: usize,
    ordinary_set_len_calls: usize,
    readonly_mutating_open_rejections: usize,
    readonly_handle_mutation_rejections: usize,
    namespace_mutation_rejections: usize,
    open_handles: usize,
    closed_handles: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ScratchFs(Arc<Mutex<ScratchState>>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScratchSnapshot {
    pub ordinary: Vec<u8>,
    pub readonly: Vec<u8>,
    pub ordinary_write_calls: usize,
    pub ordinary_bytes_written: usize,
    pub ordinary_truncate_opens: usize,
    pub ordinary_set_len_calls: usize,
    pub readonly_mutating_open_rejections: usize,
    pub readonly_handle_mutation_rejections: usize,
    pub namespace_mutation_rejections: usize,
    pub open_handles: usize,
    pub closed_handles: usize,
}

impl ScratchFs {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(ScratchState::default())))
    }
    pub(crate) fn snapshot(&self) -> Result<ScratchSnapshot> {
        let state = self
            .0
            .lock()
            .map_err(|_| io::Error::other("scratch state poisoned"))?;
        Ok(ScratchSnapshot {
            ordinary: state.ordinary.clone(),
            readonly: READONLY.to_vec(),
            ordinary_write_calls: state.ordinary_write_calls,
            ordinary_bytes_written: state.ordinary_bytes_written,
            ordinary_truncate_opens: state.ordinary_truncate_opens,
            ordinary_set_len_calls: state.ordinary_set_len_calls,
            readonly_mutating_open_rejections: state.readonly_mutating_open_rejections,
            readonly_handle_mutation_rejections: state.readonly_handle_mutation_rejections,
            namespace_mutation_rejections: state.namespace_mutation_rejections,
            open_handles: state.open_handles,
            closed_handles: state.closed_handles,
        })
    }
    fn namespace_deny(&self) -> FsError {
        if let Ok(mut state) = self.0.lock() {
            state.namespace_mutation_rejections =
                state.namespace_mutation_rejections.saturating_add(1);
        }
        FsError::PermissionDenied
    }
}

impl FileOpener for ScratchFs {
    fn open(
        &self,
        path: &Path,
        options: &OpenOptionsConfig,
    ) -> std::result::Result<Box<dyn VirtualFile + Send + Sync>, FsError> {
        let name = match entry(path) {
            Ok(Some(name)) if matches!(name, "ordinary.txt" | "readonly.txt") => name,
            Ok(None) if !options.would_mutate() => return Err(FsError::NotAFile),
            _ => {
                return Err(if options.would_mutate() {
                    self.namespace_deny()
                } else {
                    FsError::EntryNotFound
                });
            }
        };
        let ordinary = name == "ordinary.txt";
        if !ordinary && options.would_mutate() {
            let mut state = self.0.lock().map_err(|_| FsError::IOError)?;
            state.readonly_mutating_open_rejections =
                state.readonly_mutating_open_rejections.saturating_add(1);
            return Err(FsError::PermissionDenied);
        }
        // file_put_contents uses O_CREAT even for the existing fixed file.
        // This does not grant creation of any new name in this filesystem.
        if options.create_new
            || options.append
            || (!options.read && !options.write)
            || (options.truncate && !options.write)
        {
            return Err(self.namespace_deny());
        }
        let mut state = self.0.lock().map_err(|_| FsError::IOError)?;
        if ordinary && options.truncate {
            state.ordinary.clear();
            state.ordinary_truncate_opens = state.ordinary_truncate_opens.saturating_add(1);
        }
        state.open_handles = state.open_handles.saturating_add(1);
        drop(state);
        Ok(Box::new(MemoryHandle::new(
            Backing::Scratch(self.clone(), ordinary),
            options.read,
            ordinary && options.write,
        )))
    }
}

impl FileSystem for ScratchFs {
    fn readlink(&self, _: &Path) -> std::result::Result<PathBuf, FsError> {
        Err(FsError::InvalidInput)
    }
    fn read_dir(&self, path: &Path) -> std::result::Result<ReadDir, FsError> {
        if let Some(name) = entry(path)? {
            return Err(if matches!(name, "ordinary.txt" | "readonly.txt") {
                FsError::BaseNotDirectory
            } else {
                FsError::EntryNotFound
            });
        }
        let size = self.0.lock().map_err(|_| FsError::IOError)?.ordinary.len();
        Ok(listing([
            ("ordinary.txt", size),
            ("readonly.txt", READONLY.len()),
        ]))
    }
    fn metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        match entry(path)? {
            None => Ok(metadata(false, 0)),
            Some("ordinary.txt") => Ok(metadata(
                true,
                self.0.lock().map_err(|_| FsError::IOError)?.ordinary.len(),
            )),
            Some("readonly.txt") => Ok(metadata(true, READONLY.len())),
            _ => Err(FsError::EntryNotFound),
        }
    }
    fn symlink_metadata(&self, path: &Path) -> std::result::Result<Metadata, FsError> {
        self.metadata(path)
    }
    fn new_open_options(&self) -> OpenOptions<'_> {
        OpenOptions::new(self)
    }
    fn create_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.namespace_deny())
    }
    fn create_symlink(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.namespace_deny())
    }
    fn hard_link(&self, _: &Path, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.namespace_deny())
    }
    fn remove_dir(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.namespace_deny())
    }
    fn remove_file(&self, _: &Path) -> std::result::Result<(), FsError> {
        Err(self.namespace_deny())
    }
    fn rename<'a>(
        &'a self,
        _: &'a Path,
        _: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), FsError>> + Send + 'a>> {
        Box::pin(async { Err(self.namespace_deny()) })
    }
}

#[derive(Debug)]
enum Backing {
    Source(Arc<[u8]>),
    Scratch(ScratchFs, bool),
}
#[derive(Debug)]
struct MemoryHandle {
    backing: Backing,
    position: usize,
    readable: bool,
    writable: bool,
    closed: bool,
}
impl MemoryHandle {
    fn new(backing: Backing, readable: bool, writable: bool) -> Self {
        Self {
            backing,
            position: 0,
            readable,
            writable,
            closed: false,
        }
    }
    fn active(&self) -> io::Result<()> {
        if self.closed {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture handle closed",
            ))
        } else {
            Ok(())
        }
    }
    fn denied_write(&self) -> io::Error {
        if let Backing::Scratch(fs, false) = &self.backing {
            if let Ok(mut state) = fs.0.lock() {
                state.readonly_handle_mutation_rejections =
                    state.readonly_handle_mutation_rejections.saturating_add(1);
            }
        }
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "fixture handle mutation forbidden",
        )
    }
    fn len(&self) -> io::Result<usize> {
        match &self.backing {
            Backing::Source(bytes) => Ok(bytes.len()),
            Backing::Scratch(_, false) => Ok(READONLY.len()),
            Backing::Scratch(fs, true) => Ok(fs
                .0
                .lock()
                .map_err(|_| io::Error::other("scratch state poisoned"))?
                .ordinary
                .len()),
        }
    }
    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            if let Backing::Scratch(fs, _) = &self.backing {
                if let Ok(mut state) = fs.0.lock() {
                    state.open_handles = state.open_handles.saturating_sub(1);
                    state.closed_handles = state.closed_handles.saturating_add(1);
                }
            }
        }
    }
}
impl Drop for MemoryHandle {
    fn drop(&mut self) {
        self.close();
    }
}

impl AsyncRead for MemoryHandle {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.active()?;
        if !this.readable {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fixture handle not readable",
            )));
        }
        let amount = match &this.backing {
            Backing::Source(bytes) => {
                let position = this.position.min(bytes.len());
                let amount = buffer.remaining().min(bytes.len() - position);
                buffer.put_slice(&bytes[position..position + amount]);
                amount
            }
            Backing::Scratch(_, false) => {
                let position = this.position.min(READONLY.len());
                let amount = buffer.remaining().min(READONLY.len() - position);
                buffer.put_slice(&READONLY[position..position + amount]);
                amount
            }
            Backing::Scratch(fs, true) => {
                let state =
                    fs.0.lock()
                        .map_err(|_| io::Error::other("scratch state poisoned"))?;
                let position = this.position.min(state.ordinary.len());
                let amount = buffer.remaining().min(state.ordinary.len() - position);
                buffer.put_slice(&state.ordinary[position..position + amount]);
                amount
            }
        };
        this.position += amount;
        Poll::Ready(Ok(()))
    }
}
impl AsyncWrite for MemoryHandle {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.active()?;
        if !this.writable {
            return Poll::Ready(Err(this.denied_write()));
        }
        let Backing::Scratch(fs, true) = &this.backing else {
            return Poll::Ready(Err(this.denied_write()));
        };
        let end = this
            .position
            .checked_add(bytes.len())
            .filter(|end| *end <= MAX_SCRATCH_BYTES)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::FileTooLarge, "fixed scratch byte limit")
            })?;
        if bytes.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let mut state =
            fs.0.lock()
                .map_err(|_| io::Error::other("scratch state poisoned"))?;
        if end > state.ordinary.len() {
            state.ordinary.resize(end, 0);
        }
        state.ordinary[this.position..end].copy_from_slice(bytes);
        state.ordinary_write_calls = state.ordinary_write_calls.saturating_add(1);
        state.ordinary_bytes_written = state.ordinary_bytes_written.saturating_add(bytes.len());
        this.position = end;
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.active())
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().close();
        Poll::Ready(Ok(()))
    }
}
impl AsyncSeek for MemoryHandle {
    fn start_seek(self: Pin<&mut Self>, seek: SeekFrom) -> io::Result<()> {
        let this = self.get_mut();
        this.active()?;
        let next = match seek {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(value) => this.position as i128 + i128::from(value),
            SeekFrom::End(value) => this.len()? as i128 + i128::from(value),
        };
        let maximum = match this.backing {
            Backing::Source(_) => MAX_SOURCE_BYTES,
            _ => MAX_SCRATCH_BYTES,
        };
        if next < 0 || next > maximum as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fixture seek out of bounds",
            ));
        }
        this.position = next as usize;
        Ok(())
    }
    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        self.active()?;
        Poll::Ready(Ok(self.position as u64))
    }
}
impl VirtualFile for MemoryHandle {
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
        self.len().map_or(0, |len| len as u64)
    }
    fn is_open(&self) -> bool {
        !self.closed
    }
    fn set_len(&mut self, size: u64) -> std::result::Result<(), FsError> {
        self.active().map_err(|_| FsError::IOError)?;
        if !self.writable {
            self.denied_write();
            return Err(FsError::PermissionDenied);
        }
        if size > MAX_SCRATCH_BYTES as u64 {
            return Err(FsError::InvalidInput);
        }
        if let Backing::Scratch(fs, true) = &self.backing {
            let mut state = fs.0.lock().map_err(|_| FsError::IOError)?;
            state.ordinary.resize(size as usize, 0);
            state.ordinary_set_len_calls = state.ordinary_set_len_calls.saturating_add(1);
            Ok(())
        } else {
            Err(FsError::PermissionDenied)
        }
    }
    fn set_times(&mut self, _: Option<u64>, _: Option<u64>) -> std::result::Result<(), FsError> {
        self.denied_write();
        Err(FsError::PermissionDenied)
    }
    fn unlink(&mut self) -> std::result::Result<(), FsError> {
        Err(match &self.backing {
            Backing::Scratch(fs, _) => fs.namespace_deny(),
            _ => FsError::PermissionDenied,
        })
    }
    fn poll_read_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.active()?;
        if !self.readable {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fixture handle not readable",
            )));
        }
        Poll::Ready(Ok(self.len()?.saturating_sub(self.position)))
    }
    fn poll_write_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.active()?;
        if !self.writable {
            return Poll::Ready(Err(self.denied_write()));
        }
        Poll::Ready(Ok(MAX_SCRATCH_BYTES.saturating_sub(self.position)))
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
    fn wp() -> WpSourceFs {
        wp_source_fs(WP_NAMES.map(|name| {
            (
                name,
                Arc::<[u8]>::from(b"<?php // fixed source\n".as_slice()),
            )
        }))
        .unwrap()
    }
    #[test]
    fn wp_source_namespace_and_mutating_opens_are_confined() {
        runtime().block_on(async {
            let fs = wp();
            for path in [
                "../plugin.php",
                "/etc/passwd",
                "nested/plugin.php",
                "C:/plugin.php",
                "\\plugin.php",
            ] {
                assert!(fs.new_open_options().read(true).open(path).is_err());
            }
            assert!(
                fs.new_open_options()
                    .write(true)
                    .open("plugin.php")
                    .is_err()
            );
            assert!(
                fs.new_open_options()
                    .read(true)
                    .create(true)
                    .open("plugin.php")
                    .is_err()
            );
            assert!(fs.create_dir(Path::new("new")).is_err());
            assert!(
                fs.hard_link(Path::new("plugin.php"), Path::new("alias"))
                    .is_err()
            );
            assert!(
                fs.rename(Path::new("plugin.php"), Path::new("alias"))
                    .await
                    .is_err()
            );
            let mut file = fs.new_open_options().read(true).open("plugin.php").unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).await.unwrap();
            assert_eq!(bytes, b"<?php // fixed source\n");
            assert!(file.write_all(b"changed").await.is_err());
            assert_eq!(file.set_len(0), Err(FsError::PermissionDenied));
            assert_eq!(file.unlink(), Err(FsError::PermissionDenied));
            assert_eq!(
                file.set_times(Some(1), None),
                Err(FsError::PermissionDenied)
            );
        });
    }
    #[test]
    fn scratch_regular_binary_io_and_truncate_are_request_local() {
        runtime().block_on(async {
            let fs = ScratchFs::new();
            let other = ScratchFs::new();
            assert!(fs.snapshot().unwrap().ordinary.is_empty());
            let mut file = fs
                .new_open_options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open("ordinary.txt")
                .unwrap();
            file.write_all(ORDINARY).await.unwrap();
            file.rewind().await.unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).await.unwrap();
            assert_eq!(bytes, ORDINARY);
            assert_eq!(file.set_len(2), Ok(()));
            assert_eq!(fs.snapshot().unwrap().ordinary, b"or");
            file.rewind().await.unwrap();
            file.write_all(ORDINARY).await.unwrap();
            drop(file);
            let observed = fs.clone().snapshot().unwrap();
            assert_eq!(observed.ordinary, ORDINARY);
            assert_eq!(observed.readonly, READONLY);
            assert_eq!(observed.ordinary_write_calls, 2);
            assert_eq!(observed.ordinary_bytes_written, 24);
            assert_eq!(observed.ordinary_truncate_opens, 1);
            assert_eq!(observed.ordinary_set_len_calls, 1);
            assert_eq!(observed.open_handles, 0);
            assert_eq!(observed.closed_handles, 1);
            assert!(other.snapshot().unwrap().ordinary.is_empty());
            assert_eq!(other.snapshot().unwrap().ordinary_write_calls, 0);
        });
    }
    #[test]
    fn scratch_readonly_open_and_handle_mutations_are_rejected() {
        runtime().block_on(async {
            let fs = ScratchFs::new();
            assert!(
                fs.new_open_options()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open("readonly.txt")
                    .is_err()
            );
            let mut file = fs
                .new_open_options()
                .read(true)
                .open("readonly.txt")
                .unwrap();
            assert!(file.write_all(b"tampered").await.is_err());
            assert_eq!(file.set_len(0), Err(FsError::PermissionDenied));
            assert_eq!(
                file.set_times(Some(1), None),
                Err(FsError::PermissionDenied)
            );
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).await.unwrap();
            assert_eq!(bytes, READONLY);
            drop(file);
            let observed = fs.snapshot().unwrap();
            assert_eq!(observed.readonly, READONLY);
            assert_eq!(observed.readonly_mutating_open_rejections, 1);
            assert_eq!(observed.readonly_handle_mutation_rejections, 3);
            assert!(observed.ordinary.is_empty());
            assert_eq!(observed.open_handles, 0);
        });
    }
    #[test]
    fn scratch_namespace_and_seek_limits_reject_escape() {
        runtime().block_on(async {
            let fs = ScratchFs::new();
            for path in [
                "../ordinary.txt",
                "new.txt",
                "nested/ordinary.txt",
                "/etc/passwd",
            ] {
                assert!(
                    fs.new_open_options()
                        .write(true)
                        .create(true)
                        .open(path)
                        .is_err()
                );
            }
            assert!(
                fs.new_open_options()
                    .write(true)
                    .create_new(true)
                    .open("ordinary.txt")
                    .is_err()
            );
            assert!(fs.remove_file(Path::new("ordinary.txt")).is_err());
            assert!(
                fs.create_symlink(Path::new("ordinary.txt"), Path::new("alias"))
                    .is_err()
            );
            assert!(
                fs.rename(Path::new("ordinary.txt"), Path::new("elsewhere"))
                    .await
                    .is_err()
            );
            let mut file = fs
                .new_open_options()
                .read(true)
                .write(true)
                .open("ordinary.txt")
                .unwrap();
            assert!(file.seek(SeekFrom::Start(u64::MAX)).await.is_err());
            assert!(file.seek(SeekFrom::Current(-1)).await.is_err());
            assert_eq!(
                file.set_len((MAX_SCRATCH_BYTES + 1) as u64),
                Err(FsError::InvalidInput)
            );
            file.seek(SeekFrom::Start(MAX_SCRATCH_BYTES as u64))
                .await
                .unwrap();
            assert!(file.write_all(b"x").await.is_err());
            drop(file);
            assert!(fs.snapshot().unwrap().ordinary.is_empty());
            assert_eq!(fs.snapshot().unwrap().namespace_mutation_rejections, 8);
        });
    }
}

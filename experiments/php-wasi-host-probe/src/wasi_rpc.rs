//! Three bounded WASIp1 interceptors; every ordinary operation uses original WASI.
use super::{
    State,
    descriptor_bridge::{CHANNEL_NAME, REQUEST_LIMIT},
};
use anyhow::{Context, Result, ensure};
use wasmtime::{Caller, Extern, Instance, Linker, Memory, Module, Store, TypedFunc};

type OpenArgs = (u32, u32, u32, u32, u32, u64, u64, u32, u32);
type WriteArgs = (u32, u32, u32, u32);
pub struct Delegates {
    open: TypedFunc<OpenArgs, u32>,
    write: TypedFunc<WriteArgs, u32>,
    close: TypedFunc<u32, u32>,
}
pub struct Original {
    open: Extern,
    write: Extern,
    close: Extern,
}
fn memory(caller: &mut Caller<'_, State>) -> Result<Memory> {
    caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .context("missing caller memory")
}
fn word(memory: Memory, caller: &Caller<'_, State>, address: u32) -> Result<u32> {
    let mut bytes = [0; 4];
    memory.read(caller, address as usize, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
pub fn intercept(linker: &mut Linker<State>, store: &mut Store<State>) -> Result<Original> {
    let original = Original {
        open: linker
            .get(&mut *store, "wasi_snapshot_preview1", "path_open")
            .context("missing WASI path_open")?,
        write: linker
            .get(&mut *store, "wasi_snapshot_preview1", "fd_write")
            .context("missing WASI fd_write")?,
        close: linker
            .get(&mut *store, "wasi_snapshot_preview1", "fd_close")
            .context("missing WASI fd_close")?,
    };
    linker.allow_shadowing(true);
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "path_open",
        |mut caller: Caller<'_, State>,
         fd: u32,
         flags: u32,
         path: u32,
         length: u32,
         oflags: u32,
         rights: u64,
         inheriting: u64,
         fdflags: u32,
         result: u32|
         -> Result<u32> {
            // The RPC directory is explicitly the first WASIp1 preopen, fd 3.
            let mem = memory(&mut caller)?;
            let channel = if fd == 3 && length as usize == CHANNEL_NAME.len() {
                let mut bytes = [0; 7];
                mem.read(&caller, path as usize, &mut bytes)?;
                bytes == CHANNEL_NAME
            } else {
                false
            };
            if channel {
                ensure!(
                    caller.data().channel_fd.is_none(),
                    "RPC channel already open"
                );
            }
            let delegate = caller
                .data()
                .delegates
                .as_ref()
                .context("WASI delegate not ready")?
                .open
                .clone();
            let status = delegate.call(
                &mut caller,
                (
                    fd, flags, path, length, oflags, rights, inheriting, fdflags, result,
                ),
            )?;
            if channel && status == 0 {
                caller.data_mut().channel_fd = Some(word(mem, &caller, result)?);
            }
            Ok(status)
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |mut caller: Caller<'_, State>,
         fd: u32,
         iov: u32,
         count: u32,
         written: u32|
         -> Result<u32> {
            if caller.data().channel_fd != Some(fd) {
                let delegate = caller
                    .data()
                    .delegates
                    .as_ref()
                    .context("WASI delegate not ready")?
                    .write
                    .clone();
                return delegate.call(&mut caller, (fd, iov, count, written));
            }
            ensure!(count <= 16, "RPC iovec count limit");
            let mem = memory(&mut caller)?;
            word(mem, &caller, written)?; // Validate result address before host effects.
            let mut request = Vec::new();
            for index in 0..count {
                let descriptor = iov.checked_add(index * 8).context("RPC iovec overflow")?;
                let pointer = word(mem, &caller, descriptor)?;
                let length = word(
                    mem,
                    &caller,
                    descriptor.checked_add(4).context("RPC iovec overflow")?,
                )? as usize;
                ensure!(
                    length <= REQUEST_LIMIT.saturating_sub(request.len()),
                    "RPC write limit"
                );
                let start = request.len();
                request.resize(start + length, 0);
                mem.read(&caller, pointer as usize, &mut request[start..])?;
            }
            caller.data().bridge.receive(&request)?;
            mem.write(
                &mut caller,
                written as usize,
                &(request.len() as u32).to_le_bytes(),
            )?;
            Ok(0)
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_close",
        |mut caller: Caller<'_, State>, fd: u32| -> Result<u32> {
            let delegate = caller
                .data()
                .delegates
                .as_ref()
                .context("WASI delegate not ready")?
                .close
                .clone();
            let result = delegate.call(&mut caller, fd)?;
            if result == 0 && caller.data().channel_fd == Some(fd) {
                caller.data_mut().channel_fd = None;
            }
            Ok(result)
        },
    )?;
    Ok(original)
}
pub fn bind(store: &mut Store<State>, php: Instance, original: Original) -> Result<()> {
    let module = Module::new(
        store.engine(),
        include_str!("../fixtures/wasi-forwarder.wat"),
    )?;
    let memory = php
        .get_memory(&mut *store, "memory")
        .context("missing PHP memory")?;
    let mut linker = Linker::new(store.engine());
    linker.define(&mut *store, "php", "memory", memory)?;
    linker.define(&mut *store, "original", "path_open", original.open)?;
    linker.define(&mut *store, "original", "fd_write", original.write)?;
    linker.define(&mut *store, "original", "fd_close", original.close)?;
    let instance = linker.instantiate(&mut *store, &module)?;
    let delegates = Delegates {
        open: instance.get_typed_func(&mut *store, "path_open")?,
        write: instance.get_typed_func(&mut *store, "fd_write")?,
        close: instance.get_typed_func(&mut *store, "fd_close")?,
    };
    store.data_mut().delegates = Some(delegates);
    Ok(())
}

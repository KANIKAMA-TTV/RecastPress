//! Only the canonical two-member STORE ZIP emitted by the workbench.
//! This is not a generic extractor, approval/signature or publication operation.
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECEIPT_BYTES: usize = 1024;
const MAX_ARCHIVE_BYTES: usize = MAX_PAYLOAD_BYTES + MAX_RECEIPT_BYTES + 220;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PackageZipError(());
impl fmt::Debug for PackageZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackageZipError")
    }
}
impl fmt::Display for PackageZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("package unpack failed")
    }
}
impl std::error::Error for PackageZipError {}

struct Members<'a> {
    payload: &'a [u8],
    receipt: &'a [u8],
}
struct Member<'a> {
    bytes: &'a [u8],
    crc: u32,
    offset: u32,
}
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], PackageZipError> {
        let end = self.at.checked_add(count).ok_or(PackageZipError(()))?;
        let bytes = self.bytes.get(self.at..end).ok_or(PackageZipError(()))?;
        self.at = end;
        Ok(bytes)
    }
    fn exact(&mut self, expected: &[u8]) -> Result<(), PackageZipError> {
        if self.take(expected.len())? != expected {
            return Err(PackageZipError(()));
        }
        Ok(())
    }
    fn u16(&mut self) -> Result<u16, PackageZipError> {
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| PackageZipError(()))?;
        Ok(u16::from_le_bytes(bytes))
    }
    fn u32(&mut self) -> Result<u32, PackageZipError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| PackageZipError(()))?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn equals16(&mut self, value: u16) -> Result<(), PackageZipError> {
        if self.u16()? != value {
            return Err(PackageZipError(()));
        }
        Ok(())
    }
    fn equals32(&mut self, value: u32) -> Result<(), PackageZipError> {
        if self.u32()? != value {
            return Err(PackageZipError(()));
        }
        Ok(())
    }
}

// RFC1952 CRC-32/IEEE (not a cryptographic hash/authenticator). Payload identity
// still requires the existing native SHA-256 receipt and bundle admission.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
        }
    }
    crc ^ u32::MAX
}

fn local<'a>(
    cursor: &mut Cursor<'a>,
    name: &[u8],
    cap: usize,
) -> Result<Member<'a>, PackageZipError> {
    let offset = u32::try_from(cursor.at).map_err(|_| PackageZipError(()))?;
    cursor.equals32(0x0403_4b50)?;
    cursor.equals16(20)?; // version needed
    cursor.equals16(0)?; // flags
    cursor.equals16(0)?; // method STORE
    cursor.equals16(0)?; // time 00:00
    cursor.equals16(33)?; // date 1980-01-01
    let crc = cursor.u32()?;
    let size = cursor.u32()?;
    cursor.equals32(size)?; // no compression
    cursor.equals16(u16::try_from(name.len()).map_err(|_| PackageZipError(()))?)?;
    cursor.equals16(0)?; // extra length
    cursor.exact(name)?;
    let length = usize::try_from(size).map_err(|_| PackageZipError(()))?;
    if length > cap {
        return Err(PackageZipError(()));
    }
    let bytes = cursor.take(length)?;
    if crc32(bytes) != crc {
        return Err(PackageZipError(()));
    }
    Ok(Member { bytes, crc, offset })
}

fn central(
    cursor: &mut Cursor<'_>,
    name: &[u8],
    member: &Member<'_>,
) -> Result<(), PackageZipError> {
    cursor.equals32(0x0201_4b50)?;
    cursor.equals16(20)?; // version made by, DOS
    cursor.equals16(20)?;
    cursor.equals16(0)?;
    cursor.equals16(0)?;
    cursor.equals16(0)?;
    cursor.equals16(33)?;
    cursor.equals32(member.crc)?;
    let size = u32::try_from(member.bytes.len()).map_err(|_| PackageZipError(()))?;
    cursor.equals32(size)?;
    cursor.equals32(size)?;
    cursor.equals16(u16::try_from(name.len()).map_err(|_| PackageZipError(()))?)?;
    cursor.equals16(0)?; // extra
    cursor.equals16(0)?; // comment
    cursor.equals16(0)?; // disk
    cursor.equals16(0)?; // internal attributes
    cursor.equals32(0)?; // external attributes
    cursor.equals32(member.offset)?;
    cursor.exact(name)
}

fn parse(bytes: &[u8]) -> Result<Members<'_>, PackageZipError> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(PackageZipError(()));
    }
    let mut cursor = Cursor { bytes, at: 0 };
    let payload = local(&mut cursor, b"bundle.json", MAX_PAYLOAD_BYTES)?;
    let receipt = local(&mut cursor, b"receipt.json", MAX_RECEIPT_BYTES)?;
    let central_start = cursor.at;
    central(&mut cursor, b"bundle.json", &payload)?;
    central(&mut cursor, b"receipt.json", &receipt)?;
    let central_size = cursor
        .at
        .checked_sub(central_start)
        .ok_or(PackageZipError(()))?;
    cursor.equals32(0x0605_4b50)?;
    cursor.equals16(0)?;
    cursor.equals16(0)?;
    cursor.equals16(2)?;
    cursor.equals16(2)?;
    cursor.equals32(u32::try_from(central_size).map_err(|_| PackageZipError(()))?)?;
    cursor.equals32(u32::try_from(central_start).map_err(|_| PackageZipError(()))?)?;
    cursor.equals16(0)?;
    if cursor.at != bytes.len() {
        return Err(PackageZipError(()));
    }
    Ok(Members {
        payload: payload.bytes,
        receipt: receipt.bytes,
    })
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file); // std provides no close-error acknowledgement.
    Ok(())
}

fn source_regular(metadata: &fs::Metadata) -> bool {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}

fn unpack_with(
    input: &Path,
    output_new_dir: &Path,
    admit: impl FnOnce(&[u8], &[u8]) -> Result<(), PackageZipError>,
    mut write: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> Result<(), PackageZipError> {
    // Refuse existing final entries (including dangling links) before input IO.
    match fs::symlink_metadata(output_new_dir) {
        Ok(_) => return Err(PackageZipError(())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(PackageZipError(())),
    }
    if output_new_dir.file_name().is_none() {
        return Err(PackageZipError(()));
    }
    let parent = output_new_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err(PackageZipError(()));
    }
    let source = fs::symlink_metadata(input).map_err(|_| PackageZipError(()))?;
    if !source_regular(&source) {
        return Err(PackageZipError(()));
    }
    let file = File::open(input).map_err(|_| PackageZipError(()))?;
    if !file.metadata().map_err(|_| PackageZipError(()))?.is_file() {
        return Err(PackageZipError(()));
    }
    let mut archive = Vec::new();
    file.take((MAX_ARCHIVE_BYTES as u64) + 1)
        .read_to_end(&mut archive)
        .map_err(|_| PackageZipError(()))?;
    let members = parse(&archive)?;
    // Borrow the original single-read buffer. No path reopen, reserialization or
    // success merely because ZIP CRCs agree. Admission completes before mkdir.
    admit(members.payload, members.receipt)?;
    fs::create_dir(output_new_dir).map_err(|_| PackageZipError(()))?;
    write(&output_new_dir.join("bundle.json"), members.payload).map_err(|_| PackageZipError(()))?;
    write(&output_new_dir.join("receipt.json"), members.receipt)
        .map_err(|_| PackageZipError(()))?;
    Ok(())
}

/// The only file names written are host constants, never archive paths.
/// All CRCs and receipt SHA/length/admission pass before creating the output dir.
/// Failure retains partial output, without atomic-pair/parent-durability/close-ACK
/// or hostile-ancestor, complete allocation/RSS/deadline, approval or publish claims.
pub fn unpack_package(input: &Path, output_new_dir: &Path) -> Result<(), PackageZipError> {
    unpack_with(
        input,
        output_new_dir,
        |payload, receipt| {
            // Drop the admitted PreparedSite before writing. Nothing is published.
            drop(
                crate::bundle_receipt::verify_payload(payload, receipt)
                    .map_err(|_| PackageZipError(()))?,
            );
            Ok(())
        },
        write_new,
    )
}

#[cfg(test)]
#[path = "package_zip_contract.rs"]
mod tests;

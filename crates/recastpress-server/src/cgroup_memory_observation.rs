//! Candidate raw memory observations at one caller-selected cgroup v2 level.
//!
//! The caller selects the directory. This does not discover process membership,
//! mounts, namespace identity or ancestors, prove that files belong to cgroupfs,
//! or derive effective headroom or an allocation allowance. Ancestor constraints,
//! cgroup v1 and Windows job objects remain unimplemented. No cache/CLI policy,
//! automatic formula or sharing across processes is connected to these data.
//!
//! The three reads are non-atomic. memory.current includes the selected cgroup's
//! descendants. memory.max may be exceeded temporarily; memory.high throttles
//! and reclaims rather than directly triggering OOM, and can also be exceeded.
//! Therefore no ordering between the three values is imposed. These counters
//! are observations, not authorization capabilities or process RSS guarantees.
//!
//! Source: https://docs.kernel.org/admin-guide/cgroup-v2.html (memory interface).

#[cfg(any(target_os = "linux", test))]
use std::io::Read;
use std::{io, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CgroupV2LevelObservation {
    pub current_bytes: u64,
    pub max_bytes: Option<u64>,
    pub high_bytes: Option<u64>,
}

/// Read memory.current, memory.max and memory.high at exactly this directory.
/// Values are bytes; None means the file contained `max`, not that a read failed
/// or that ancestor limits are absent. Missing or malformed files are errors.
#[cfg(target_os = "linux")]
pub fn observe_cgroup_v2_level(directory: &Path) -> io::Result<CgroupV2LevelObservation> {
    read_level(directory)
}

/// No implementation is claimed for cgroup v1 or non-Linux resource controls.
#[cfg(not(target_os = "linux"))]
pub fn observe_cgroup_v2_level(_directory: &Path) -> io::Result<CgroupV2LevelObservation> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "cgroup v2 level observation is currently implemented only for Linux",
    ))
}

#[cfg(any(target_os = "linux", test))]
const MAX_FILE_BYTES: usize = 128;

#[cfg(any(target_os = "linux", test))]
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(any(target_os = "linux", test))]
fn read_bounded(reader: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    // The extra byte detects overflow without reading an unbounded file/reader.
    reader
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(invalid("cgroup memory file exceeds its 128-byte bound"));
    }
    Ok(bytes)
}

#[cfg(any(target_os = "linux", test))]
fn token(input: &[u8]) -> io::Result<&[u8]> {
    if input.len() > MAX_FILE_BYTES {
        return Err(invalid("cgroup memory file exceeds its 128-byte bound"));
    }
    // Kernel single-value files conventionally end in one LF. Accept no other
    // whitespace, extra lines or units; this is not a generic trim operation.
    let value = input.strip_suffix(b"\n").unwrap_or(input);
    if value.is_empty() {
        return Err(invalid("cgroup memory value is missing"));
    }
    Ok(value)
}

#[cfg(any(target_os = "linux", test))]
fn decimal(value: &[u8]) -> io::Result<u64> {
    let mut result = 0u64;
    for &byte in value {
        if !byte.is_ascii_digit() {
            return Err(invalid("cgroup memory value is not ASCII unsigned decimal"));
        }
        result = result
            .checked_mul(10)
            .and_then(|result| result.checked_add(u64::from(byte - b'0')))
            .ok_or_else(|| invalid("cgroup memory integer overflow"))?;
    }
    Ok(result)
}

#[cfg(any(target_os = "linux", test))]
fn parse_current(input: &[u8]) -> io::Result<u64> {
    decimal(token(input)?)
}

#[cfg(any(target_os = "linux", test))]
fn parse_limit(input: &[u8]) -> io::Result<Option<u64>> {
    let value = token(input)?;
    if value == b"max" {
        Ok(None)
    } else {
        decimal(value).map(Some)
    }
}

#[cfg(any(target_os = "linux", test))]
fn read_level(directory: &Path) -> io::Result<CgroupV2LevelObservation> {
    let current_bytes = parse_current(&read_bounded(std::fs::File::open(
        directory.join("memory.current"),
    )?)?)?;
    let max_bytes = parse_limit(&read_bounded(std::fs::File::open(
        directory.join("memory.max"),
    )?)?)?;
    let high_bytes = parse_limit(&read_bounded(std::fs::File::open(
        directory.join("memory.high"),
    )?)?)?;
    Ok(CgroupV2LevelObservation {
        current_bytes,
        max_bytes,
        high_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_values_preserve_zero_max_and_full_unsigned_range() {
        for (input, expected) in [
            (b"0".as_slice(), 0),
            (b"1\n", 1),
            (b"0001024\n", 1024),
            (b"18446744073709551615\n", u64::MAX),
        ] {
            assert_eq!(parse_current(input).unwrap(), expected);
            assert_eq!(parse_limit(input).unwrap(), Some(expected));
        }
        assert_eq!(parse_limit(b"max").unwrap(), None);
        assert_eq!(parse_limit(b"max\n").unwrap(), None);
        assert!(parse_current(b"max\n").is_err());
    }

    #[test]
    fn malformed_multiline_non_ascii_and_overflow_are_not_unlimited() {
        for input in [
            b"".as_slice(),
            b"\n",
            b"-1",
            b"+1",
            b"1.0",
            b"1e3",
            b"1 kB",
            b"1B",
            b" 1",
            b"1 ",
            b"1\t",
            b"1\r\n",
            b"1\n2",
            b"1\n\n",
            b"max\n0",
            b"MAX",
            b"max ",
            b"\xff",
            b"\0",
            b"18446744073709551616",
        ] {
            assert_eq!(
                parse_current(input).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(
                parse_limit(input).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn readers_stop_at_one_sentinel_byte_and_propagate_errors() {
        let exact = vec![b'0'; MAX_FILE_BYTES];
        assert_eq!(read_bounded(exact.as_slice()).unwrap(), exact);
        assert_eq!(parse_current(&exact).unwrap(), 0);
        let oversized = vec![b'0'; MAX_FILE_BYTES + 1];
        assert_eq!(
            read_bounded(oversized.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            parse_limit(&oversized).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut endless = io::repeat(b'0');
        assert_eq!(
            read_bounded(&mut endless).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "fixture read error",
                ))
            }
        }
        assert_eq!(
            read_bounded(Broken).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let suffix = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "recastpress-cgroup-observation-{}-{time}-{suffix}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, bytes: &[u8]) {
            std::fs::write(self.0.join(name), bytes).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Remove only the three files this fixture creates, never recurse.
            for name in ["memory.current", "memory.max", "memory.high"] {
                let _ = std::fs::remove_file(self.0.join(name));
            }
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn local_files_accept_unordered_counters_and_fail_on_missing_or_bad_input() {
        let fixture = Fixture::new();
        fixture.write("memory.current", b"100\n");
        fixture.write("memory.max", b"10\n");
        fixture.write("memory.high", b"20\n");
        assert_eq!(
            read_level(&fixture.0).unwrap(),
            CgroupV2LevelObservation {
                current_bytes: 100,
                max_bytes: Some(10),
                high_bytes: Some(20),
            }
        );
        fixture.write("memory.max", b"max\n");
        fixture.write("memory.high", b"0\n");
        assert_eq!(
            read_level(&fixture.0).unwrap(),
            CgroupV2LevelObservation {
                current_bytes: 100,
                max_bytes: None,
                high_bytes: Some(0),
            }
        );
        fixture.write("memory.high", b"max\n");
        assert_eq!(read_level(&fixture.0).unwrap().high_bytes, None);
        std::fs::remove_file(fixture.0.join("memory.high")).unwrap();
        assert_eq!(
            read_level(&fixture.0).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        fixture.write("memory.high", b"max\n");
        fixture.write("memory.current", b"max\n");
        assert_eq!(
            read_level(&fixture.0).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fixture.write("memory.current", &[b'0'; MAX_FILE_BYTES + 1]);
        assert_eq!(
            read_level(&fixture.0).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_reports_unsupported_without_assuming_an_unlimited_cgroup() {
        assert_eq!(
            observe_cgroup_v2_level(Path::new("not-a-cgroup-directory"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
    }
}

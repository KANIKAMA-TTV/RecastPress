//! Candidate read-only observation of host physical-memory counters.
//!
//! Linux MemTotal is usable host RAM; MemAvailable is a kernel estimate of
//! memory available without swapping. Neither is a process/container allocation
//! allowance. This does not inspect cgroups, Windows job objects, process RSS or
//! shared-pool headroom, calculate budgets, or connect CLI/cache settings. Do not
//! use these host observations directly as a product memory allocation policy.
//!
//! Source: https://docs.kernel.org/filesystems/proc.html (meminfo), and
//! https://github.com/torvalds/linux/blob/master/fs/proc/meminfo.c (kB units).
//! Windows remains unsupported: direct FFI would violate the workspace's unsafe
//! prohibition, and no new safe-wrapper dependency has been adopted.

use std::io;
#[cfg(any(target_os = "linux", test))]
use std::io::Read;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalMemoryObservation {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// Reads host counters once, without reserving memory or selecting cache limits.
/// Missing MemAvailable is an error; MemFree is never substituted for it.
#[cfg(target_os = "linux")]
pub fn observe_physical_memory() -> io::Result<PhysicalMemoryObservation> {
    let input = read_bounded(std::fs::File::open("/proc/meminfo")?)?;
    parse_meminfo(&input)
}

/// No safe platform implementation has been adopted on this OS.
#[cfg(not(target_os = "linux"))]
pub fn observe_physical_memory() -> io::Result<PhysicalMemoryObservation> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "physical-memory observation is currently implemented only for Linux",
    ))
}

#[cfg(any(target_os = "linux", test))]
const MAX_MEMINFO_BYTES: usize = 64 * 1024;

#[cfg(any(target_os = "linux", test))]
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(any(target_os = "linux", test))]
fn read_bounded(reader: impl Read) -> io::Result<Vec<u8>> {
    let mut input = Vec::new();
    // One extra byte is read only to detect overflow; accepted input is at most
    // 64 KiB. A longer or nonterminating reader cannot cause an unbounded read.
    reader
        .take((MAX_MEMINFO_BYTES + 1) as u64)
        .read_to_end(&mut input)?;
    if input.len() > MAX_MEMINFO_BYTES {
        return Err(invalid("meminfo exceeds the 64 KiB input limit"));
    }
    Ok(input)
}

#[cfg(any(target_os = "linux", test))]
fn parse_meminfo(input: &[u8]) -> io::Result<PhysicalMemoryObservation> {
    if input.len() > MAX_MEMINFO_BYTES {
        return Err(invalid("meminfo exceeds the 64 KiB input limit"));
    }
    let text = std::str::from_utf8(input).map_err(|_| invalid("meminfo is not UTF-8"))?;
    let mut total = None;
    let mut available = None;
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(':') else {
            if matches!(
                line.split_ascii_whitespace().next(),
                Some("MemTotal" | "MemAvailable")
            ) {
                return Err(invalid("meminfo field is missing its colon"));
            }
            continue;
        };
        let destination = match name.trim_ascii() {
            "MemTotal" => &mut total,
            "MemAvailable" => &mut available,
            _ => continue,
        };
        if destination.is_some() {
            return Err(invalid("duplicate required meminfo field"));
        }
        let mut words = rest.split_ascii_whitespace();
        let value = words
            .next()
            .ok_or_else(|| invalid("missing meminfo value"))?;
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid("meminfo value is not an unsigned decimal integer"));
        }
        if words.next() != Some("kB") || words.next().is_some() {
            return Err(invalid("meminfo value requires exactly the kB unit"));
        }
        let kibibytes = value
            .parse::<u64>()
            .map_err(|_| invalid("meminfo integer overflow"))?;
        *destination = Some(
            kibibytes
                .checked_mul(1024)
                .ok_or_else(|| invalid("meminfo byte conversion overflow"))?,
        );
    }
    let total_bytes = total.ok_or_else(|| invalid("missing MemTotal"))?;
    let available_bytes = available.ok_or_else(|| invalid("missing MemAvailable"))?;
    if total_bytes == 0 || available_bytes > total_bytes {
        return Err(invalid("inconsistent physical-memory counters"));
    }
    Ok(PhysicalMemoryObservation {
        total_bytes,
        available_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_uses_available_not_free_and_checks_binary_kilobytes() {
        assert_eq!(
            parse_meminfo(
                b"MemFree: 1 kB\nMemAvailable:\t3072 kB\nMemTotal: 4096 kB\nHugePages_Total: 0\n"
            )
            .unwrap(),
            PhysicalMemoryObservation {
                total_bytes: 4 * 1024 * 1024,
                available_bytes: 3 * 1024 * 1024,
            }
        );
        assert_eq!(
            parse_meminfo(b"MemTotal: 1 kB\nMemAvailable: 0 kB\n")
                .unwrap()
                .available_bytes,
            0
        );
    }

    #[test]
    fn meminfo_refuses_missing_duplicate_malformed_and_overflowing_counters() {
        for input in [
            "",
            "MemTotal: 1 kB\nMemFree: 1 kB",
            "MemAvailable: 1 kB",
            "MemTotal: 1 kB\nMemTotal: 1 kB\nMemAvailable: 0 kB",
            "MemTotal: 1 kB\nMemTotal:1 kB\nMemAvailable: 0 kB",
            "MemTotal: 1 kB\nMemAvailable: 0 kB\nMemAvailable: 0 kB",
            "MemTotal 1 kB\nMemTotal: 1 kB\nMemAvailable: 0 kB",
            "MemTotal: -1 kB\nMemAvailable: 0 kB",
            "MemTotal: +1 kB\nMemAvailable: 0 kB",
            "MemTotal: 1.5 kB\nMemAvailable: 0 kB",
            "MemTotal: 1\nMemAvailable: 0 kB",
            "MemTotal: 1 KB\nMemAvailable: 0 kB",
            "MemTotal: 1 kB extra\nMemAvailable: 0 kB",
            "MemTotal: 18446744073709551616 kB\nMemAvailable: 0 kB",
            "MemTotal: 18014398509481984 kB\nMemAvailable: 0 kB",
            "MemTotal: 0 kB\nMemAvailable: 0 kB",
            "MemTotal: 1 kB\nMemAvailable: 2 kB",
            "MemTotal: 1 kB\nMemAvailable: -1 kB",
        ] {
            assert_eq!(
                parse_meminfo(input.as_bytes()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        assert_eq!(
            parse_meminfo(b"\xff").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn meminfo_read_is_bounded_and_accepts_the_exact_limit() {
        let exact = vec![b'x'; MAX_MEMINFO_BYTES];
        assert_eq!(read_bounded(exact.as_slice()).unwrap(), exact);
        let oversized = vec![b'x'; MAX_MEMINFO_BYTES + 1];
        assert_eq!(
            read_bounded(oversized.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            parse_meminfo(&oversized).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut endless = io::repeat(b'x');
        assert_eq!(
            read_bounded(&mut endless).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn other_operating_systems_do_not_claim_a_physical_memory_observation() {
        assert_eq!(
            observe_physical_memory().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }
}

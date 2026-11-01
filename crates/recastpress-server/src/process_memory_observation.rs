//! Explicit, read-only sampling of this process's Linux smaps-rollup RSS.
//!
//! Source: https://docs.kernel.org/filesystems/proc.html (smaps and smaps_rollup).
//! RSS includes shared mappings and excludes separately reported hugetlb usage.
//! It is neither PSS, cgroup charge, logical budget usage nor allocation headroom.
//! A page-table walk is not an instantaneous atomic observation, and bounding
//! returned bytes does not bound the time spent opening/reading the proc file.
//! No monitoring loop, fallback source, CLI/cache connection or capacity formula
//! is provided. Native Linux execution remains outside the current validation.

use std::io;
#[cfg(any(target_os = "linux", test))]
use std::io::Read;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessRssObservation {
    /// The kernel's Rss field converted from kB (1024 bytes), not total RAM use.
    pub resident_bytes: u64,
}

/// Sample only /proc/self/smaps_rollup on explicit invocation.
/// Missing, inaccessible or malformed data is an error, never zero or fallback.
#[cfg(target_os = "linux")]
pub fn observe_self_smaps_rollup_rss() -> io::Result<ProcessRssObservation> {
    let input = read_bounded(std::fs::File::open("/proc/self/smaps_rollup")?)?;
    parse_rollup(&input)
}

#[cfg(not(target_os = "linux"))]
pub fn observe_self_smaps_rollup_rss() -> io::Result<ProcessRssObservation> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process smaps-rollup RSS observation is implemented only for Linux",
    ))
}

#[cfg(any(target_os = "linux", test))]
const MAX_ROLLUP_BYTES: usize = 64 * 1024;

#[cfg(any(target_os = "linux", test))]
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(any(target_os = "linux", test))]
fn read_bounded(reader: impl Read) -> io::Result<Vec<u8>> {
    let mut input = Vec::new();
    reader
        .take((MAX_ROLLUP_BYTES + 1) as u64)
        .read_to_end(&mut input)?;
    if input.len() > MAX_ROLLUP_BYTES {
        return Err(invalid("smaps_rollup exceeds the 64 KiB input limit"));
    }
    Ok(input)
}

#[cfg(any(target_os = "linux", test))]
fn parse_rollup(input: &[u8]) -> io::Result<ProcessRssObservation> {
    if input.len() > MAX_ROLLUP_BYTES {
        return Err(invalid("smaps_rollup exceeds the 64 KiB input limit"));
    }
    let mut resident = None;
    for line in input.split(|byte| *byte == b'\n') {
        let Some(colon) = line.iter().position(|byte| *byte == b':') else {
            if line
                .split(|byte| byte.is_ascii_whitespace())
                .find(|part| !part.is_empty())
                == Some(b"Rss".as_slice())
            {
                return Err(invalid("Rss field is missing its colon"));
            }
            continue;
        };
        if line[..colon].trim_ascii() != b"Rss" {
            // Header and unrelated fields are not part of this parser's domain.
            continue;
        }
        if resident.is_some() {
            return Err(invalid("duplicate Rss field"));
        }
        let mut words = line[colon + 1..]
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|word| !word.is_empty());
        let digits = words.next().ok_or_else(|| invalid("missing Rss value"))?;
        if !digits.iter().all(u8::is_ascii_digit) {
            return Err(invalid("Rss requires an unsigned decimal integer"));
        }
        if words.next() != Some(b"kB".as_slice()) || words.next().is_some() {
            return Err(invalid("Rss requires exactly the kB unit"));
        }
        let value = digits
            .iter()
            .try_fold(0u64, |value, byte| {
                value.checked_mul(10)?.checked_add(u64::from(byte - b'0'))
            })
            .ok_or_else(|| invalid("Rss integer overflow"))?;
        resident = Some(
            value
                .checked_mul(1024)
                .ok_or_else(|| invalid("Rss byte conversion overflow"))?,
        );
    }
    Ok(ProcessRssObservation {
        resident_bytes: resident.ok_or_else(|| invalid("missing Rss field"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_only_parsing_accepts_zero_and_ignores_unrelated_header_and_fields() {
        let sample =
            b"arbitrary header\xff\0\nPss: not parsed\nRss:\t00123 kB\nPrivate_Hugetlb: 8 kB\n";
        assert_eq!(parse_rollup(sample).unwrap().resident_bytes, 123 * 1024);
        assert_eq!(parse_rollup(b"Rss: 0 kB").unwrap().resident_bytes, 0);
        let largest = u64::MAX / 1024;
        assert_eq!(
            parse_rollup(format!("Rss: {largest} kB\n").as_bytes())
                .unwrap()
                .resident_bytes,
            largest * 1024
        );
    }

    #[test]
    fn required_field_is_unique_strict_and_checked_without_substitution() {
        for input in [
            b"VmRSS: 1 kB\n".as_slice(),
            b"Rss: 1 kB\nRss: 1 kB\n",
            b"Rss 1 kB\n",
            b"Rss: -1 kB",
            b"Rss: +1 kB",
            b"Rss: 1.5 kB",
            b"Rss: \xff kB",
            b"Rss:",
            b"Rss: 1 KB",
            b"Rss: 1",
            b"Rss: 1 kB extra",
            b"Rss: 18446744073709551616 kB",
            b"Rss: 18014398509481984 kB",
        ] {
            assert_eq!(
                parse_rollup(input).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn bounded_read_checks_one_sentinel_byte_and_preserves_io_errors() {
        let mut exact = b"Rss: 1 kB\n".to_vec();
        exact.resize(MAX_ROLLUP_BYTES, b'x');
        assert_eq!(
            parse_rollup(&read_bounded(exact.as_slice()).unwrap())
                .unwrap()
                .resident_bytes,
            1024
        );
        exact.extend_from_slice(&[b'x'; 32]);
        let mut reader = io::Cursor::new(exact);
        assert_eq!(
            read_bounded(&mut reader).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(reader.position(), (MAX_ROLLUP_BYTES + 1) as u64);
        assert_eq!(
            parse_rollup(&vec![b'x'; MAX_ROLLUP_BYTES + 1])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        struct Failed;
        impl Read for Failed {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "fixture denial",
                ))
            }
        }
        assert_eq!(
            read_bounded(Failed).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}

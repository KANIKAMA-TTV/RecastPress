//! Byte-input subset of WordPress `wp_kses_no_null`.

use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SlashZeroMode {
    /// The source's default/exact `slash_zero = "remove"` option.
    #[default]
    Remove,
    /// The explicit source option `slash_zero = "keep"`.
    Keep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoNullError {
    /// Final output byte cap, checked length overflow or output allocation failure.
    OutputLimit,
}

impl fmt::Display for NoNullError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("null removal exceeds the host output byte or allocation bound")
    }
}

impl std::error::Error for NoNullError {}

struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    length: usize,
    limit: usize,
}

impl Writer<'_> {
    fn repeat(&mut self, byte: u8, count: usize) -> Result<(), NoNullError> {
        self.length = self
            .length
            .checked_add(count)
            .filter(|length| *length <= self.limit)
            .ok_or(NoNullError::OutputLimit)?;
        if let Some(output) = &mut self.output {
            output.resize(self.length, byte);
        }
        Ok(())
    }
}

fn scan(input: &[u8], mode: SlashZeroMode, writer: &mut Writer<'_>) -> Result<(), NoNullError> {
    let mut filtered = input
        .iter()
        .copied()
        .filter(|byte| !matches!(byte, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f))
        .peekable();
    while let Some(byte) = filtered.next() {
        if mode == SlashZeroMode::Remove && byte == b'\\' {
            let mut count = 1usize;
            while filtered.peek() == Some(&b'\\') {
                filtered.next();
                count = count.checked_add(1).ok_or(NoNullError::OutputLimit)?;
            }
            if filtered.peek() == Some(&b'0') {
                while filtered.peek() == Some(&b'0') {
                    filtered.next();
                }
            } else {
                writer.repeat(b'\\', count)?;
            }
        } else {
            writer.repeat(byte, 1)?;
        }
    }
    Ok(())
}

/// WordPress 7.1.3 `kses.php:2020` for byte input and explicit Remove/Keep.
/// First remove 00..08/0B/0C/0E..1F, then (Remove only) remove runs of one or
/// more backslashes followed by one or more ASCII zeros in that filtered stream.
/// TAB/LF/CR, DEL and high bytes are retained. No Unicode decoding is performed.
///
/// The same iterator traversal measures and emits output, with no intermediate
/// Vec. Final length is checked before reservation, so shrinking input need not
/// fit the final byte cap before filtering. No partial result is returned.
/// OutputLimit includes overflow or allocation failure; this host cap is not PHP
/// behavior, a product capacity setting or shared RAM/RSS accounting.
///
/// Source option coercions, non-byte inputs, PCRE resource failures, recursive
/// replacements, entity decoding, general HTML sanitization/escaping and output
/// safety are unmodeled. No site settings, public HTTP or raw DB path is used.
pub fn wp_kses_no_null_bytes(
    input: &[u8],
    mode: SlashZeroMode,
    max_output_bytes: usize,
) -> Result<Vec<u8>, NoNullError> {
    let mut count = Writer {
        output: None,
        length: 0,
        limit: usize::MAX,
    };
    scan(input, mode, &mut count)?;
    if count.length > max_output_bytes {
        return Err(NoNullError::OutputLimit);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(count.length)
        .map_err(|_| NoNullError::OutputLimit)?;
    let mut writer = Writer {
        output: Some(&mut output),
        length: 0,
        limit: count.length,
    };
    scan(input, mode, &mut writer)?;
    if writer.length != count.length {
        return Err(NoNullError::OutputLimit);
    }
    Ok(output)
}

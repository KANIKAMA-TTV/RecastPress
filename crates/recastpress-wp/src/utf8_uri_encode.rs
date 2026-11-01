//! Fixed utf8_uri_encode for wholly valid UTF-8 Bytes and typed nonnegative length.
//! This is a literal byte encoder, not a URL validator, HTML escape or slug API.

use crate::wp_is_valid_utf8_bytes;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Utf8UriEncodeError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for Utf8UriEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "input is outside the valid UTF-8 URI-encoding subset",
            Self::OutputLimit => "URI-encoded output exceeds the host byte limit",
            Self::AllocationFailed => "URI-encoded output allocation failed",
        })
    }
}
impl std::error::Error for Utf8UriEncodeError {}

fn unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}
fn source_next(
    written: usize,
    token: usize,
    length: usize,
) -> Result<Option<usize>, Utf8UriEncodeError> {
    match written.checked_add(token) {
        Some(next) if length == 0 || next <= length => Ok(Some(next)),
        Some(_) => Ok(None),
        None if length != 0 => Ok(None),
        None => Err(Utf8UriEncodeError::OutputLimit),
    }
}
fn host_next(written: usize, token: usize, limit: usize) -> Result<usize, Utf8UriEncodeError> {
    written
        .checked_add(token)
        .filter(|next| *next <= limit)
        .ok_or(Utf8UriEncodeError::OutputLimit)
}
fn output_buffer(length: usize) -> Result<Vec<u8>, Utf8UriEncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| Utf8UriEncodeError::AllocationFailed)?;
    Ok(output)
}

// Only called after whole-input validation. On valid UTF-8, the source's octet
// collection is equivalent to one complete scalar token (2/3/4 bytes).
fn walk(
    raw: &[u8],
    length: usize,
    encode_ascii: bool,
    mut emit: impl FnMut(&[u8]) -> Result<(), Utf8UriEncodeError>,
) -> Result<(), Utf8UriEncodeError> {
    const UPPER: &[u8; 16] = b"0123456789ABCDEF";
    const LOWER: &[u8; 16] = b"0123456789abcdef";
    let mut offset = 0;
    let mut written = 0;
    while offset < raw.len() {
        let first = raw[offset];
        let mut token = [0u8; 12];
        let (consumed, size) = if first < 128 {
            if !encode_ascii || unreserved(first) {
                token[0] = first;
                (1, 1)
            } else {
                token[..3].copy_from_slice(&[
                    b'%',
                    UPPER[usize::from(first >> 4)],
                    UPPER[usize::from(first & 15)],
                ]);
                (1, 3)
            }
        } else {
            let width = if first < 224 {
                2
            } else if first < 240 {
                3
            } else {
                4
            };
            for (index, byte) in raw[offset..offset + width].iter().enumerate() {
                token[index * 3] = b'%';
                token[index * 3 + 1] = LOWER[usize::from(byte >> 4)];
                token[index * 3 + 2] = LOWER[usize::from(byte & 15)];
            }
            (width, width * 3)
        };
        let Some(next) = source_next(written, size, length)? else {
            break;
        };
        emit(&token[..size])?;
        written = next;
        offset += consumed;
    }
    Ok(())
}

/// Pinned WP formatting.php:1160 with Bytes, whole valid UTF-8, usize length,
/// and strict bool. All input is validated, including bytes beyond the source
/// truncation point; invalid input is UnsupportedValue, never a partial prefix.
///
/// ASCII is copied exactly when encode_ascii=false, including NUL/control/'%'.
/// With true, rawurlencode's ASCII unreserved set stays literal, other ASCII
/// becomes uppercase %XX. Every byte of a non-ASCII scalar becomes lowercase
/// %xx. No normalization, trim, URL validity, HTML escaping or identity policy.
///
/// Source length0 is unlimited. Nonzero length counts encoded bytes, stopping
/// at the first whole token that does not fit, without skipping ahead. The host
/// max_output is separate: count through the same walker, enforce the final cap,
/// fallibly reserve exact logical length, then emit. No partial result returns.
/// Input length is not a cap test. Allocation capacity/RSS/peak/shared budget,
/// concurrency and source PHP coercion/invalid-UTF-8 behavior are not modeled.
pub fn utf8_uri_encode_utf8_bytes(
    raw: &[u8],
    length: usize,
    encode_ascii: bool,
    max_output: usize,
) -> Result<Vec<u8>, Utf8UriEncodeError> {
    if !wp_is_valid_utf8_bytes(raw) {
        return Err(Utf8UriEncodeError::UnsupportedValue);
    }
    let mut measured = 0;
    walk(raw, length, encode_ascii, |token| {
        measured = host_next(measured, token.len(), max_output)?;
        Ok(())
    })?;
    let mut output = output_buffer(measured)?;
    walk(raw, length, encode_ascii, |token| {
        output.extend_from_slice(token);
        Ok(())
    })?;
    debug_assert_eq!(output.len(), measured);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_budget_overflow_stops_while_unlimited_and_host_overflow_are_errors() {
        assert_eq!(source_next(usize::MAX - 1, 3, usize::MAX), Ok(None));
        assert_eq!(
            source_next(usize::MAX - 1, 3, 0),
            Err(Utf8UriEncodeError::OutputLimit)
        );
        assert_eq!(
            source_next(usize::MAX - 1, 1, usize::MAX),
            Ok(Some(usize::MAX))
        );
        assert_eq!(
            host_next(usize::MAX, 1, usize::MAX),
            Err(Utf8UriEncodeError::OutputLimit)
        );
        assert_eq!(host_next(0, 0, 0), Ok(0));
    }
    #[test]
    fn capacity_overflow_is_a_fixed_allocation_failure_without_attempting_giant_input() {
        assert_eq!(
            output_buffer(usize::MAX),
            Err(Utf8UriEncodeError::AllocationFailed)
        );
    }
}

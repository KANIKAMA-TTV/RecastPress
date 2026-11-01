//! Conditional-decoded-UTF-8 subset of pinned post.php:5745.
use crate::{Utf8UriEncodeError, utf8_uri_encode_utf8_bytes};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TruncatePostSlugError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for TruncatePostSlugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "decoded value is outside the UTF-8 truncation subset",
            Self::OutputLimit => "truncated slug exceeds the host byte limit",
            Self::AllocationFailed => "truncated slug allocation failed",
        })
    }
}
impl std::error::Error for TruncatePostSlugError {}
impl From<Utf8UriEncodeError> for TruncatePostSlugError {
    fn from(error: Utf8UriEncodeError) -> Self {
        match error {
            Utf8UriEncodeError::UnsupportedValue => Self::UnsupportedValue,
            Utf8UriEncodeError::OutputLimit => Self::OutputLimit,
            Utf8UriEncodeError::AllocationFailed => Self::AllocationFailed,
        }
    }
}

fn buffer(length: usize) -> Result<Vec<u8>, TruncatePostSlugError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| TruncatePostSlugError::AllocationFailed)?;
    Ok(result)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// Each original byte is consumed once. Decoded '+'/'%' are never reprocessed.
fn decode(raw: &[u8]) -> Result<Vec<u8>, TruncatePostSlugError> {
    let mut result = buffer(raw.len())?;
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'+' {
            result.push(b' ');
        } else if raw[index] == b'%' && raw.len() - index >= 3 {
            if let (Some(high), Some(low)) = (hex(raw[index + 1]), hex(raw[index + 2])) {
                result.push(high * 16 + low);
                index += 3;
                continue;
            }
            result.push(raw[index]);
        } else {
            result.push(raw[index]);
        }
        index += 1;
    }
    Ok(result)
}

fn trimmed(raw: &[u8]) -> &[u8] {
    let end = raw
        .iter()
        .rposition(|&byte| byte != b'-')
        .map_or(0, |index| index + 1);
    &raw[..end]
}

fn copy_final(raw: &[u8], maximum: usize) -> Result<Vec<u8>, TruncatePostSlugError> {
    let selected = trimmed(raw);
    if selected.len() > maximum {
        return Err(TruncatePostSlugError::OutputLimit);
    }
    let mut result = buffer(selected.len())?;
    result.extend_from_slice(selected);
    Ok(result)
}

/// Only raw.len()>length invokes PHP-style nonrecursive urldecode. If decoding
/// is unchanged, byte-substring is used; otherwise the entire decoded value
/// must be valid UTF-8 before URI encoding with encode_ascii=true. No UTF-8
/// guard applies to the unchanged/substr or no-truncation branches.
/// Length0 retains URI0-unlimited behavior in the changed-decoding branch.
/// ASCII '-' is always trimmed from the end after branch selection. The host
/// cap applies only then; intermediate allocations/RSS/peak are unbudgeted.
/// No sanitization, lowercase, URL validity, uniqueness or publication policy.
pub fn truncate_post_slug_conditional_decoded_utf8_bytes(
    raw: &[u8],
    length: usize,
    max_output: usize,
) -> Result<Vec<u8>, TruncatePostSlugError> {
    if raw.len() <= length {
        return copy_final(raw, max_output);
    }
    let decoded = decode(raw)?;
    if decoded == raw {
        return copy_final(&raw[..length], max_output);
    }
    let mut encoded = utf8_uri_encode_utf8_bytes(&decoded, length, true, usize::MAX)?;
    let final_length = trimmed(&encoded).len();
    if final_length > max_output {
        return Err(TruncatePostSlugError::OutputLimit);
    }
    encoded.truncate(final_length);
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity_failure_and_uri_errors_have_closed_classification() {
        assert_eq!(
            buffer(usize::MAX),
            Err(TruncatePostSlugError::AllocationFailed)
        );
        assert_eq!(
            TruncatePostSlugError::from(Utf8UriEncodeError::UnsupportedValue),
            TruncatePostSlugError::UnsupportedValue
        );
        assert_eq!(decode(b"%252b+%2B%zz%").unwrap(), b"%2b +%zz%");
    }
}

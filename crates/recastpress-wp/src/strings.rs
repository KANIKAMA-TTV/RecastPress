//! Byte-preserving WordPress string transforms. These functions are not HTML,
//! JSON, URL, or SQL escaping interfaces and do not make a value safe to output.

use crate::{
    CompatibilityError,
    value::{PhpArray, PhpValue},
};

const MAX_ARRAY_DEPTH: usize = 64;

/// Byte-string kernel of WordPress normalize_whitespace: PHP's default trim,
/// CR-to-LF replacement, then LF-run and space/tab-run collapse, in that order.
/// Interior NUL/VT/FF, non-UTF-8 bytes and Unicode bytes are preserved.
/// This is neither output escaping nor Unicode normalization; PHP coercion of
/// non-byte-string inputs is not implemented.
pub fn normalize_whitespace_bytes(input: &[u8]) -> Vec<u8> {
    let is_trim = |byte: u8| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b);
    let mut start = 0;
    let mut end = input.len();
    while start < end && is_trim(input[start]) {
        start += 1;
    }
    while end > start && is_trim(input[end - 1]) {
        end -= 1;
    }
    let mut result = Vec::with_capacity(end - start);
    for &byte in &input[start..end] {
        let normalized = match byte {
            b'\r' | b'\n' => b'\n',
            b' ' | b'\t' => b' ',
            byte => byte,
        };
        if matches!(normalized, b'\n' | b' ') && result.last() == Some(&normalized) {
            continue;
        }
        result.push(normalized);
    }
    result
}

/// wp_basename for byte-string path and suffix. WordPress URL-encodes the path
/// before applying basename, but does NOT encode the suffix. Suffix matching
/// therefore happens against encoded ASCII and may even split a percent escape.
/// Both slash spellings delimit segments. This does not access or authorize files.
pub fn wp_basename_bytes(path: &[u8], suffix: &[u8]) -> Vec<u8> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = Vec::new();
    for &byte in path {
        match byte {
            b'/' | b'\\' => encoded.push(b'/'),
            b' ' => encoded.push(b'+'),
            byte if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') => {
                encoded.push(byte);
            }
            byte => encoded.extend_from_slice(&[
                b'%',
                HEX[(byte >> 4) as usize],
                HEX[(byte & 15) as usize],
            ]),
        }
    }
    let end = encoded
        .iter()
        .rposition(|&byte| byte != b'/')
        .map_or(0, |last| last + 1);
    let start = encoded[..end]
        .iter()
        .rposition(|&byte| byte == b'/')
        .map_or(0, |last| last + 1);
    let mut base = &encoded[start..end];
    if !suffix.is_empty() && suffix.len() < base.len() && base.ends_with(suffix) {
        base = &base[..base.len() - suffix.len()];
    }
    decode_form_bytes(base)
}

pub(crate) fn decode_form_bytes(input: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%'
            && index + 2 < input.len()
            && let (Some(high), Some(low)) =
                (hex_digit(input[index + 1]), hex_digit(input[index + 2]))
        {
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(if input[index] == b'+' {
                b' '
            } else {
                input[index]
            });
            index += 1;
        }
    }
    decoded
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Byte-string input kernel. Removes both forward/backward trailing slashes;
/// NUL, whitespace, and all other bytes remain unchanged.
pub fn untrailingslashit_bytes(value: &[u8]) -> Vec<u8> {
    let end = value
        .iter()
        .rposition(|byte| !matches!(byte, b'/' | b'\\'))
        .map_or(0, |index| index + 1);
    value[..end].to_vec()
}

pub fn trailingslashit_bytes(value: &[u8]) -> Vec<u8> {
    let mut result = untrailingslashit_bytes(value);
    result.push(b'/');
    result
}

/// wp_normalize_path's byte-string, non-stream subset. Values containing ://
/// require the PHP wrapper registry and are explicitly unsupported here.
/// Preserves source regex behavior after LF and at the start of network shares.
/// This transforms spelling only; it does not resolve or authorize a file path.
pub fn wp_normalize_path_bytes(path: &[u8]) -> Result<Vec<u8>, CompatibilityError> {
    if path.windows(3).any(|part| part == b"://") {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let mut result = Vec::with_capacity(path.len());
    let mut index = 0;
    while index < path.len() {
        let byte = if path[index] == b'\\' {
            b'/'
        } else {
            path[index]
        };
        result.push(byte);
        if byte == b'/' && index > 0 && path[index - 1] != b'\n' {
            index += 1;
            while index < path.len() && matches!(path[index], b'/' | b'\\') {
                index += 1;
            }
        } else {
            index += 1;
        }
    }
    if result.get(1) == Some(&b':') {
        result[0].make_ascii_uppercase();
    }
    Ok(result)
}

/// wp_slash for modeled scalars and arrays. Keys and non-string scalars remain
/// unchanged. Object/resource/reference behavior is not modeled.
pub fn wp_slash(value: &PhpValue) -> Result<PhpValue, CompatibilityError> {
    transform(value, 0, add_slashes)
}

/// wp_unslash for modeled scalars and arrays, preserving keys and byte strings.
/// The object traversal in the source's map_deep is intentionally unsupported.
pub fn wp_unslash(value: &PhpValue) -> Result<PhpValue, CompatibilityError> {
    transform(value, 0, strip_slashes)
}

fn transform(
    value: &PhpValue,
    depth: usize,
    strings: fn(&[u8]) -> Vec<u8>,
) -> Result<PhpValue, CompatibilityError> {
    if depth > MAX_ARRAY_DEPTH {
        return Err(CompatibilityError::RecursionLimit);
    }
    Ok(match value {
        PhpValue::Unsupported => return Err(CompatibilityError::UnsupportedValue),
        PhpValue::Bytes(bytes) => PhpValue::Bytes(strings(bytes)),
        PhpValue::Array(array) => {
            let mut result = PhpArray::new();
            for (key, value) in array.iter() {
                result.insert(key.clone(), transform(value, depth + 1, strings)?);
            }
            PhpValue::Array(result)
        }
        value => value.clone(),
    })
}

fn add_slashes(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    for byte in bytes {
        match byte {
            0 => result.extend_from_slice(b"\\0"),
            b'\'' | b'"' | b'\\' => result.extend_from_slice(&[b'\\', *byte]),
            byte => result.push(*byte),
        }
    }
    result
}

fn strip_slashes(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut input = bytes.iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'\\' {
            if let Some(next) = input.next() {
                result.push(if next == b'0' { 0 } else { next });
            }
        } else {
            result.push(byte);
        }
    }
    result
}

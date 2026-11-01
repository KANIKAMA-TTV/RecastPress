//! WordPress UUID spelling checks, not ID generation or authenticity checks.

use crate::{CompatibilityError, value::PhpValue};

/// Version None uses WP's unrestricted lowercase hex pattern, not RFC version
/// or variant validation. Some(4) enforces WP's v4/variant pattern. Other numeric
/// versions and their _doing_it_wrong diagnostics are unsupported for strings.
/// Non-string input returns false BEFORE version validation, as the source does.
pub fn wp_is_uuid(input: &PhpValue, version: Option<i64>) -> Result<bool, CompatibilityError> {
    let PhpValue::Bytes(bytes) = input else {
        return Ok(false);
    };
    if !matches!(version, None | Some(4)) {
        return Err(CompatibilityError::UnsupportedValue);
    }
    // The source's PCRE $ also matches immediately before one final LF.
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.len() != 36 {
        return Ok(false);
    }
    let shape = bytes.iter().enumerate().all(|(index, &byte)| {
        if matches!(index, 8 | 13 | 18 | 23) {
            byte == b'-'
        } else {
            byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
        }
    });
    Ok(shape
        && (version.is_none()
            || (bytes[14] == b'4' && matches!(bytes[19], b'8' | b'9' | b'a' | b'b'))))
}

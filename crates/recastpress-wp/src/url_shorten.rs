//! Byte-input subset of WordPress 7.1.3 formatting.php:6318 (`url_shorten`).

use crate::{CompatibilityError, untrailingslashit_bytes};

/// Removes each source substring in its own case-sensitive, nonrecursive pass,
/// trims trailing slash/backslash bytes, and applies the source's byte cutoff.
/// Signed 64-bit lengths preserve PHP's negative substring-length behavior.
/// PHP coercion of noninteger lengths and other URL input types is unsupported.
/// This neither parses URLs nor escapes output; UTF-8 may be cut mid-sequence.
pub fn url_shorten_bytes(url: &[u8], length: i64) -> Result<Vec<u8>, CompatibilityError> {
    let mut stripped = url.to_vec();
    for pattern in [b"https://".as_slice(), b"http://", b"www."] {
        let mut next = Vec::with_capacity(stripped.len());
        let mut index = 0;
        while index < stripped.len() {
            if stripped[index..].starts_with(pattern) {
                index += pattern.len();
            } else {
                next.push(stripped[index]);
                index += 1;
            }
        }
        stripped = next;
    }
    let mut shortened = untrailingslashit_bytes(&stripped);
    // Every negative length enters the source branch, even for empty input.
    // A positive length wider than usize cannot truncate a representable slice.
    let truncate = if length < 0 {
        true
    } else {
        usize::try_from(length).is_ok_and(|limit| shortened.len() > limit)
    };
    if truncate {
        // PHP promotes the bottom three subtraction results to float, rounded
        // to PHP_INT_MIN on the fixed 64-bit oracle. Every possible byte slice
        // is shorter than its magnitude, so all three yield an empty prefix.
        let substring_length = length.saturating_sub(3);
        let keep = if substring_length < 0 {
            usize::try_from(substring_length.unsigned_abs())
                .map_or(0, |remove| shortened.len().saturating_sub(remove))
        } else {
            usize::try_from(substring_length).expect("positive truncated length fits usize")
        };
        shortened.truncate(keep);
        shortened.extend_from_slice(b"&hellip;");
    }
    Ok(shortened)
}

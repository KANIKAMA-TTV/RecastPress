//! Byte-string subsets of WordPress 7.1.3 formatting.php:6339, :6366, :6388.
//! These reproduce narrow hex-color rules, not CSS parsing or output escaping.

/// Returns the original bytes for empty input or a matched `#RGB`/`#RRGGBB`.
///
/// The source regex is `|^#([A-Fa-f0-9]{3}){1,2}$|`; one final LF is accepted
/// and retained. `None` represents PHP null and differs from `Some(empty)`.
/// Scalar coercions, PHP diagnostics and PCRE resource failures are not modeled.
pub fn sanitize_hex_color_bytes(color: &[u8]) -> Option<Vec<u8>> {
    if color.is_empty() {
        return Some(Vec::new());
    }
    if valid_hashed_color(color) {
        Some(color.to_vec())
    } else {
        None
    }
}

/// Removes every leading `#`, preserving case and an accepted final LF.
/// Invalid bytes return PHP-null-equivalent `None`; hashes alone return empty.
pub fn sanitize_hex_color_no_hash_bytes(color: &[u8]) -> Option<Vec<u8>> {
    let unhashed = without_hashes(color);
    if unhashed.is_empty() || valid_digits(unhashed) {
        Some(unhashed.to_vec())
    } else {
        None
    }
}

/// Adds one `#` only for a valid nonempty unhashed hex color.
///
/// The source tests PHP truthiness of `sanitize_hex_color_no_hash`. Its only
/// false byte result is empty: a valid result has three/six digits, optionally
/// followed by LF. Invalid inputs and inputs consisting only of hashes retain
/// their original bytes. This function does not make arbitrary CSS safe.
pub fn maybe_hash_hex_color_bytes(color: &[u8]) -> Vec<u8> {
    let unhashed = without_hashes(color);
    if valid_digits(unhashed) {
        let mut result = Vec::with_capacity(unhashed.len() + 1);
        result.push(b'#');
        result.extend_from_slice(unhashed);
        result
    } else {
        color.to_vec()
    }
}

fn without_hashes(color: &[u8]) -> &[u8] {
    let start = color
        .iter()
        .position(|&byte| byte != b'#')
        .unwrap_or(color.len());
    &color[start..]
}

fn valid_hashed_color(color: &[u8]) -> bool {
    color.strip_prefix(b"#").is_some_and(valid_digits)
}

fn valid_digits(color: &[u8]) -> bool {
    let digits = color.strip_suffix(b"\n").unwrap_or(color);
    matches!(digits.len(), 3 | 6) && digits.iter().all(u8::is_ascii_hexdigit)
}

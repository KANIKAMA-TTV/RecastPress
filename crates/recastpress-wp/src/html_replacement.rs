//! Byte-only subset of WordPress 7.1.3 formatting.php:757.
//! This replaces bytes in regex-captured delimiters; it is not output escaping.

use crate::{
    CompatibilityError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_html_split_bytes,
};

/// Replaces bytes only inside the odd entries of WordPress's HTML split.
///
/// The entire map is validated before allocating output or splitting input.
/// Only byte keys and byte values are supported. Canonical numeric keys become
/// PHP integer keys and are unsupported. A single empty key is an identity
/// operation; an empty key among multiple pairs is unsupported because PHP emits
/// warnings. Other unsupported values are rejected even when no delimiter/key
/// would match: PHP's broader coercions and lazy diagnostics are not modeled.
pub fn wp_replace_in_html_tags_bytes(
    haystack: &[u8],
    pairs: &PhpArray,
) -> Result<Vec<u8>, CompatibilityError> {
    for (key, value) in pairs.iter() {
        let (PhpKey::Bytes(needle), PhpValue::Bytes(_)) = (key, value) else {
            return Err(CompatibilityError::UnsupportedValue);
        };
        if pairs.len() > 1 && needle.is_empty() {
            return Err(CompatibilityError::UnsupportedValue);
        }
    }
    if pairs.is_empty()
        || matches!(pairs.iter().next(), Some((PhpKey::Bytes(needle), _)) if needle.is_empty())
    {
        return Ok(haystack.to_vec());
    }

    let parts = wp_html_split_bytes(haystack);
    let mut result = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index % 2 == 0 {
            result.extend_from_slice(part);
        } else {
            replace_delimiter(part, pairs, &mut result);
        }
    }
    Ok(result)
}

fn replace_delimiter(input: &[u8], pairs: &PhpArray, result: &mut Vec<u8>) {
    let mut cursor = 0;
    while cursor < input.len() {
        let mut longest: Option<(&[u8], &[u8])> = None;
        for (key, value) in pairs.iter() {
            // All entries were validated before any output was allocated.
            if let (PhpKey::Bytes(needle), PhpValue::Bytes(replacement)) = (key, value)
                && input[cursor..].starts_with(needle)
                && longest.is_none_or(|(previous, _)| needle.len() > previous.len())
            {
                longest = Some((needle, replacement));
            }
        }
        if let Some((needle, replacement)) = longest {
            // Advance only through original input: inserted bytes are not
            // rescanned or split, for either str_replace or strtr behavior.
            result.extend_from_slice(replacement);
            cursor += needle.len();
        } else {
            result.push(input[cursor]);
            cursor += 1;
        }
    }
}

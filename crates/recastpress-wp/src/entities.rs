//! Fixed-default HTML entity normalization, not an HTML sanitizer.

#[path = "entity_names.rs"]
mod names;
#[path = "xml_entity_mapping.rs"]
mod xml_mapping;

use crate::valid_unicode_i64;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityNormalizeError {
    /// Final output byte cap, checked length overflow or output allocation failure.
    OutputLimit,
}

impl fmt::Display for EntityNormalizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("entity normalization exceeds the host output byte or allocation bound")
    }
}

impl std::error::Error for EntityNormalizeError {}

struct Writer<'a> {
    bytes: Option<&'a mut Vec<u8>>,
    length: usize,
    limit: usize,
}

impl Writer<'_> {
    fn emit(&mut self, part: &[u8]) -> Result<(), EntityNormalizeError> {
        self.length = self
            .length
            .checked_add(part.len())
            .filter(|length| *length <= self.limit)
            .ok_or(EntityNormalizeError::OutputLimit)?;
        if let Some(bytes) = &mut self.bytes {
            bytes.extend_from_slice(part);
        }
        Ok(())
    }
}

struct Numeric<'a> {
    digits: &'a [u8],
    significant: &'a [u8],
    consumed: usize,
    value: i64,
    hex: bool,
}

fn numeric(tail: &[u8]) -> Option<Numeric<'_>> {
    if tail.first() != Some(&b'#') {
        return None;
    }
    let hex = matches!(tail.get(1), Some(b'x' | b'X'));
    let start = if hex { 2 } else { 1 };
    let mut end = start;
    while tail.get(end).is_some_and(|byte| {
        if hex {
            byte.is_ascii_hexdigit()
        } else {
            byte.is_ascii_digit()
        }
    }) {
        end += 1;
    }
    if tail.get(end) != Some(&b';') {
        return None;
    }
    let digits = &tail[start..end];
    let significant = &digits[digits.iter().position(|byte| *byte != b'0')?..];
    if significant.len() > if hex { 6 } else { 7 } {
        return None;
    }
    let radix = if hex { 16 } else { 10 };
    // Six hex or seven decimal significant digits fit signed i64 exactly.
    let value = significant.iter().fold(0i64, |value, &byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => byte - b'A' + 10,
        };
        value * radix + i64::from(digit)
    });
    Some(Numeric {
        digits,
        significant,
        consumed: end + 1,
        value,
        hex,
    })
}

fn named(tail: &[u8]) -> Option<usize> {
    let end = tail
        .iter()
        .take(11)
        .position(|byte| !byte.is_ascii_alphanumeric())?;
    if tail[end] != b';'
        || names::SORTED_NAMES
            .binary_search_by(|name| name.cmp(&&tail[..end]))
            .is_err()
    {
        return None;
    }
    // Every name in the checked source table satisfies the source callback's
    // [A-Za-z]{2,8}[0-9]{0,2} grammar. Unknown names need only retain &amp;.
    Some(end + 1)
}

/// Reference recognition for the private htmlspecialchars stage after the
/// fixed normalizer. HTML401 excludes apos; XML1 recognizes only XML5 names.
/// Valid normalized numeric references remain references, never characters.
pub(crate) fn normalized_reference_length(tail: &[u8], xml: bool) -> Option<usize> {
    if let Some(entity) = numeric(tail) {
        return valid_unicode_i64(entity.value).then_some(entity.consumed);
    }
    let consumed = named(tail)?;
    let name = &tail[..consumed - 1];
    let allowed = if xml {
        matches!(name, b"amp" | b"lt" | b"gt" | b"quot" | b"apos")
    } else {
        name != b"apos"
    };
    allowed.then_some(consumed)
}

fn normalize(input: &[u8], writer: &mut Writer<'_>, xml: bool) -> Result<(), EntityNormalizeError> {
    let mut offset = 0;
    while offset < input.len() {
        if input[offset] != b'&' {
            let end = input[offset..]
                .iter()
                .position(|byte| *byte == b'&')
                .map_or(input.len(), |relative| offset + relative);
            writer.emit(&input[offset..end])?;
            offset = end;
            continue;
        }
        let tail = &input[offset + 1..];
        if let Some(entity) = numeric(tail) {
            if valid_unicode_i64(entity.value) {
                writer.emit(if entity.hex { b"&#x" } else { b"&#" })?;
                if !entity.hex && entity.significant.len() < 3 {
                    writer.emit(&b"000"[..3 - entity.significant.len()])?;
                }
                writer.emit(entity.significant)?;
            } else {
                // A matched hex callback lowercases X even for invalid values;
                // unmatched/all-zero forms instead retain their original X.
                writer.emit(if entity.hex { b"&amp;#x" } else { b"&amp;#" })?;
                writer.emit(entity.digits)?;
            }
            writer.emit(b";")?;
            offset += 1 + entity.consumed;
        } else if let Some(consumed) = named(tail) {
            if xml {
                let name = &tail[..consumed - 1];
                let index = xml_mapping::SORTED_XML
                    .binary_search_by(|entry| entry.0.cmp(name))
                    .expect("checked HTML and XML tables have identical membership");
                writer.emit(xml_mapping::SORTED_XML[index].1)?;
            } else {
                writer.emit(&input[offset..offset + 1 + consumed])?;
            }
            offset += 1 + consumed;
        } else {
            writer.emit(b"&amp;")?;
            offset += 1;
        }
    }
    Ok(())
}

/// WordPress 7.1.3 `kses.php:2169` for byte input, HTML context and the fixed
/// 253 default HTML entity names. No KSES bootstrap or mutable globals are read.
///
/// Scan original ampersands nonrecursively, preserving the source's decimal,
/// hexadecimal and named normalization order. Measure with the same traversal,
/// check the final byte cap, reserve the final output, then emit complete bytes.
/// Leading-zero compression is measured exactly, not by a pessimistic 5x bound.
///
/// The cap and allocation failure are host policies, not PHP semantics, RSS or
/// shared RAM accounting. Input bytes are not UTF-8 sanitized. XML context,
/// customized allowlists, scalar casts, PCRE failures, PHP HTML conversion
/// tables, general HTML sanitization/escaping and output safety are not modeled.
pub fn wp_kses_normalize_entities_default_html_bytes(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, EntityNormalizeError> {
    normalize_output(input, max_output_bytes, false)
}

/// Explicit WordPress 7.1.3 XML context, fixed XML5/HTML253 names, PHP 8.5.12
/// ENT_HTML5 and UTF-8 charset observations. XML5 stay references; the other
/// allowed names emit observed UTF-8 bytes. This does not choose site charset.
///
/// Numeric references use the same canonicalization as the HTML API, without
/// decoding them to characters. Only original ampersands are visited: emitted
/// bytes are never decoded again. Other input, including invalid UTF-8, passes
/// through. Exact final counting and emission share one traversal; allocation
/// and cap failures return no partial output. Output is unpooled, with no RSS,
/// working-capacity or shared-budget accounting. General HTML5 decoding, XML
/// validity, custom globals, other charsets, PHP casts/diagnostics, PCRE failure,
/// sanitization, escaping and output safety are outside this profile.
pub fn wp_kses_normalize_entities_default_xml_utf8_bytes(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, EntityNormalizeError> {
    normalize_output(input, max_output_bytes, true)
}

fn normalize_output(
    input: &[u8],
    max_output_bytes: usize,
    xml: bool,
) -> Result<Vec<u8>, EntityNormalizeError> {
    let mut count = Writer {
        bytes: None,
        length: 0,
        limit: usize::MAX,
    };
    normalize(input, &mut count, xml)?;
    if count.length > max_output_bytes {
        return Err(EntityNormalizeError::OutputLimit);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(count.length)
        .map_err(|_| EntityNormalizeError::OutputLimit)?;
    let mut writer = Writer {
        bytes: Some(&mut output),
        length: 0,
        limit: count.length,
    };
    normalize(input, &mut writer, xml)?;
    if writer.length != count.length {
        return Err(EntityNormalizeError::OutputLimit);
    }
    Ok(output)
}

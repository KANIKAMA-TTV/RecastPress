//! Explicit UTF-8 `_wp_specialchars` subset, not a general PHP HTML encoder.
//!
//! The typed styles represent six selected source arguments, not arbitrary ENT
//! masks/coercions. Site charset, get_option, filters and esc_html/esc_attr's
//! upstream invalid-UTF8 processing are not modeled or selected here.

use crate::{
    entities::normalized_reference_length, wp_is_valid_utf8_bytes,
    wp_kses_normalize_entities_default_html_bytes,
    wp_kses_normalize_entities_default_xml_utf8_bytes,
};
use std::{borrow::Cow, fmt};

/// Exact selected `_wp_specialchars` quote arguments. Xml1 means literal
/// ENT_XML1 (16), which WP normalizes to ENT_QUOTES|ENT_XML1 internally.
/// Passing the raw composite value 19 to WP would instead select HTML Quotes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecialCharsQuoteStyle {
    NoQuotes,
    Compat,
    Quotes,
    Single,
    Double,
    Xml1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecialCharsError {
    /// Final output cap, checked length overflow or intermediate/output allocation failure.
    OutputLimit,
}

impl fmt::Display for SpecialCharsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("special character output exceeds the host byte or allocation bound")
    }
}
impl std::error::Error for SpecialCharsError {}

struct Writer<'a> {
    bytes: Option<&'a mut Vec<u8>>,
    length: usize,
    limit: usize,
}
impl Writer<'_> {
    fn emit(&mut self, bytes: &[u8]) -> Result<(), SpecialCharsError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .filter(|length| *length <= self.limit)
            .ok_or(SpecialCharsError::OutputLimit)?;
        if let Some(output) = &mut self.bytes {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
}

fn special(byte: u8) -> bool {
    matches!(byte, b'&' | b'<' | b'>' | b'"' | b'\'')
}

fn escape(
    input: &[u8],
    style: SpecialCharsQuoteStyle,
    double_encode: bool,
    output: &mut Writer<'_>,
) -> Result<(), SpecialCharsError> {
    use SpecialCharsQuoteStyle::*;
    let mut offset = 0;
    while offset < input.len() {
        let byte = input[offset];
        if !special(byte) {
            let length = input[offset..]
                .iter()
                .position(|byte| special(*byte))
                .unwrap_or(input.len() - offset);
            output.emit(&input[offset..offset + length])?;
            offset += length;
            continue;
        }
        if byte == b'&'
            && !double_encode
            && let Some(length) = normalized_reference_length(&input[offset + 1..], style == Xml1)
        {
            output.emit(&input[offset..offset + 1 + length])?;
            offset += 1 + length;
            continue;
        }
        let replacement: &[u8] = match byte {
            b'&' => b"&amp;",
            b'<' => b"&lt;",
            b'>' => b"&gt;",
            b'"' if matches!(style, Compat | Quotes | Double | Xml1) => b"&quot;",
            b'\'' if style == Xml1 => b"&apos;",
            // The Single source style does this after PHP's no-quotes stage.
            // Combining that final raw-apostrophe replacement into this emit
            // does not decode or rewrite any preserved reference.
            b'\'' if matches!(style, Quotes | Single) => b"&#039;",
            _ => &input[offset..offset + 1],
        };
        output.emit(replacement)?;
        offset += 1;
    }
    Ok(())
}

/// Fixed WordPress 7.1.3 `formatting.php:945`, explicit UTF-8, six typed styles
/// and a boolean double_encode. No charset/site policy or filters are read.
///
/// Original bytes without any of &<>"' return unchanged, even if invalid UTF-8.
/// With any such byte present, invalid UTF-8 returns an empty result. This is
/// deliberately different from unconditionally applying wp_check_invalid_utf8.
/// The normalizers preserve all invalid raw bytes, so this early UTF-8 rejection
/// is equivalent in this fixed profile. Valid NUL/noncharacters are not scrubbed.
///
/// When double_encode is false, a fallibly allocated, unpooled normalizer Vec
/// precedes the final cap. HTML uses fixed HTML253 then HTML401 recognition
/// (excluding apos); Xml1 uses fixed XML5/HTML253 UTF-8 observations, then XML5
/// recognition. No output/reference is recursively decoded. The second stage
/// reads the complete normalized bytes, without assuming mappings cannot emit
/// ASCII special characters. Count and emit share that same escape traversal.
///
/// Only the final output is capped/reserved exactly; allocation failures return
/// no partial result. Input/intermediate/output capacity, allocator work, peak,
/// RSS and shared pools are not accounted. Other flags, coercions, charsets,
/// PCRE failures, general htmlspecialchars/HTML5/XML validity, output safety,
/// esc_html/esc_attr integration and public HTTP/DB use are outside this subset.
pub fn wp_specialchars_utf8_bytes(
    input: &[u8],
    style: SpecialCharsQuoteStyle,
    double_encode: bool,
    max_output_bytes: usize,
) -> Result<Vec<u8>, SpecialCharsError> {
    let has_special = input.iter().any(|byte| special(*byte));
    if has_special && !wp_is_valid_utf8_bytes(input) {
        return Ok(Vec::new());
    }
    let normalized = if has_special && !double_encode {
        Cow::Owned(
            if style == SpecialCharsQuoteStyle::Xml1 {
                wp_kses_normalize_entities_default_xml_utf8_bytes(input, usize::MAX)
            } else {
                wp_kses_normalize_entities_default_html_bytes(input, usize::MAX)
            }
            .map_err(|_| SpecialCharsError::OutputLimit)?,
        )
    } else {
        Cow::Borrowed(input)
    };
    let mut count = Writer {
        bytes: None,
        length: 0,
        limit: usize::MAX,
    };
    escape(&normalized, style, double_encode, &mut count)?;
    if count.length > max_output_bytes {
        return Err(SpecialCharsError::OutputLimit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(count.length)
        .map_err(|_| SpecialCharsError::OutputLimit)?;
    let mut output = Writer {
        bytes: Some(&mut bytes),
        length: 0,
        limit: count.length,
    };
    escape(&normalized, style, double_encode, &mut output)?;
    if output.length != count.length {
        return Err(SpecialCharsError::OutputLimit);
    }
    Ok(bytes)
}

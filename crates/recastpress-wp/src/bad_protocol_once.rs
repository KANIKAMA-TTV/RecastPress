//! Default-count C-byte subset of WordPress's single protocol pass.

use crate::{
    BadProtocolError,
    value::{PhpArray, PhpValue},
    wp_kses_bad_protocol_once2_c_bytes,
};
use std::fmt;

const TOP_INPUT_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadProtocolOnceError {
    UnsupportedProtocolValue,
    UnsupportedEntityValue,
    InputLimit,
    OutputLimit,
}

impl fmt::Display for BadProtocolOnceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedProtocolValue => "protocol allowlist contains a non-byte value",
            Self::UnsupportedEntityValue => {
                "entity decoder recognizes only values from 0 through 255"
            }
            Self::InputLimit => "protocol once input exceeds the top-level host byte bound",
            Self::OutputLimit => "protocol once exceeds the host output byte or allocation bound",
        })
    }
}

impl std::error::Error for BadProtocolOnceError {}

struct Writer<'a> {
    bytes: Option<&'a mut Vec<u8>>,
    length: usize,
    limit: usize,
}

impl Writer<'_> {
    fn emit(&mut self, bytes: &[u8]) -> Result<(), BadProtocolOnceError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .filter(|length| *length <= self.limit)
            .ok_or(BadProtocolOnceError::OutputLimit)?;
        if let Some(output) = &mut self.bytes {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
}

// Return the literal prefix matched by &#0*58 or &#x0*3a, without a semicolon.
fn colon_prefix(input: &[u8]) -> Option<(usize, bool)> {
    if !input.starts_with(b"&#") {
        return None;
    }
    let hex = matches!(input.get(2), Some(b'x' | b'X'));
    let mut end = if hex { 3 } else { 2 };
    while input.get(end) == Some(&b'0') {
        end += 1;
    }
    if hex {
        if input.get(end) != Some(&b'3') || !matches!(input.get(end + 1), Some(b'a' | b'A')) {
            return None;
        }
    } else if input.get(end) != Some(&b'5') || input.get(end + 1) != Some(&b'8') {
        return None;
    }
    Some((end + 2, hex))
}

fn repair_scan(input: &[u8], writer: &mut Writer<'_>) -> Result<(), BadProtocolOnceError> {
    let mut position = 0;
    while position < input.len() {
        if let Some((length, hex)) = colon_prefix(&input[position..]) {
            let blocked = input.get(position + length).is_some_and(|byte| {
                *byte == b';'
                    || if hex {
                        byte.is_ascii_hexdigit()
                    } else {
                        byte.is_ascii_digit()
                    }
            });
            if !blocked {
                writer.emit(&input[position..position + length])?;
                writer.emit(b";")?;
                position += length;
                continue;
            }
        }
        writer.emit(&input[position..position + 1])?;
        position += 1;
    }
    Ok(())
}

fn repair(input: &[u8]) -> Result<Vec<u8>, BadProtocolOnceError> {
    let mut count = Writer {
        bytes: None,
        length: 0,
        limit: usize::MAX,
    };
    repair_scan(input, &mut count)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count.length)
        .map_err(|_| BadProtocolOnceError::OutputLimit)?;
    let mut writer = Writer {
        bytes: Some(&mut output),
        length: 0,
        limit: count.length,
    };
    repair_scan(input, &mut writer)?;
    Ok(output)
}

fn first_delimiter(input: &[u8]) -> Option<(usize, usize)> {
    for (position, &byte) in input.iter().enumerate() {
        if byte == b':' {
            return Some((position, 1));
        }
        let tail = &input[position..];
        if let Some((length, _)) = colon_prefix(tail)
            && tail.get(length) == Some(&b';')
        {
            return Some((position, length + 1));
        }
        if tail
            .get(..7)
            .is_some_and(|part| part.eq_ignore_ascii_case(b"&colon;"))
        {
            return Some((position, 7));
        }
    }
    None
}

fn trim_suffix(input: &[u8]) -> &[u8] {
    let trim = |byte: &u8| matches!(byte, 0 | 9 | 10 | 11 | 13 | 32);
    let start = input
        .iter()
        .position(|byte| !trim(byte))
        .unwrap_or(input.len());
    let end = input
        .iter()
        .rposition(|byte| !trim(byte))
        .map_or(start, |last| last + 1);
    &input[start..end]
}

fn append(mut prefix: Vec<u8>, suffix: &[u8]) -> Result<Vec<u8>, BadProtocolOnceError> {
    let length = prefix
        .len()
        .checked_add(suffix.len())
        .ok_or(BadProtocolOnceError::OutputLimit)?;
    prefix
        .try_reserve_exact(length - prefix.len())
        .map_err(|_| BadProtocolOnceError::OutputLimit)?;
    prefix.extend_from_slice(suffix);
    Ok(prefix)
}

fn once(content: &[u8], protocols: &PhpArray, count: u8) -> Result<Vec<u8>, BadProtocolOnceError> {
    let repaired = repair(content)?;
    let Some((position, delimiter_length)) = first_delimiter(&repaired) else {
        return Ok(repaired);
    };
    let prefix = &repaired[..position];
    if prefix.windows(2).any(|part| part == b"/?") {
        return Ok(repaired);
    }
    let suffix = trim_suffix(&repaired[position + delimiter_length..]);
    let working_cap = prefix
        .len()
        .checked_add(1)
        .ok_or(BadProtocolOnceError::OutputLimit)?;
    // This call precedes the feed-depth test and only inspects a called prefix.
    let protocol =
        wp_kses_bad_protocol_once2_c_bytes(prefix, protocols, working_cap).map_err(|error| {
            match error {
                BadProtocolError::UnsupportedProtocolValue => {
                    BadProtocolOnceError::UnsupportedProtocolValue
                }
                BadProtocolError::UnsupportedEntityValue => {
                    BadProtocolOnceError::UnsupportedEntityValue
                }
                BadProtocolError::InputLimit => BadProtocolOnceError::InputLimit,
                BadProtocolError::OutputLimit => BadProtocolOnceError::OutputLimit,
            }
        })?;
    if protocol == b"feed:" {
        if count > 2 {
            return Ok(Vec::new());
        }
        let nested = once(suffix, protocols, count + 1)?;
        // PHP empty(string) also recognizes exactly "0".
        if nested.is_empty() || nested == b"0" {
            return Ok(nested);
        }
        return append(protocol, &nested);
    }
    append(protocol, suffix)
}

/// WordPress 7.1.3 `kses.php:2100`, byte content/all-byte protocol values/C,
/// with source default count=1 (at most three internal feed frames).
/// Repair incomplete numeric colon entities across the whole content, split
/// once, check raw prefix /?, trim only the suffix with PHP's six-byte trim,
/// then call once2 only for an inspected prefix. This is not URL validation.
///
/// All protocol values are prevalidated; non-byte short-circuit diagnostics and
/// arbitrary PHP count arguments are excluded. The new whole-raw-input 1MiB host
/// bound applies only at this entry, not repaired content or recursive tails.
/// Decoder 0..255/1MiB limits apply only to prefixes actually passed to once2.
/// Do not decode/filter the whole body or bypass a third-frame prefix check.
///
/// Repair, normalization and recursive output Vecs are unpooled and allocated
/// before the top final cap check. They can exceed the raw input limit after
/// repair; working allowances and final output cap are separate. All lengths and
/// reservations are checked, and failures return no partial output. This is not
/// PHP failure equivalence, RSS/shared-budget accounting, a product protocol
/// policy, HTML sanitization, authorization or public HTTP/DB integration.
pub fn wp_kses_bad_protocol_once_c_bytes(
    content: &[u8],
    allowed_protocols: &PhpArray,
    max_output_bytes: usize,
) -> Result<Vec<u8>, BadProtocolOnceError> {
    if allowed_protocols
        .iter()
        .any(|(_, value)| !matches!(value, PhpValue::Bytes(_)))
    {
        return Err(BadProtocolOnceError::UnsupportedProtocolValue);
    }
    if content.len() > TOP_INPUT_LIMIT {
        return Err(BadProtocolOnceError::InputLimit);
    }
    let output = working_default(content, allowed_protocols)?;
    if output.len() > max_output_bytes {
        return Err(BadProtocolOnceError::OutputLimit);
    }
    Ok(output)
}

/// Internal default-count engine: callers own all-value validation, their top
/// raw-input guard and their final cap. Expanded working content is not subject
/// to the public once API's entry bound. Every call resets feed count to one.
pub(crate) fn working_default(
    content: &[u8],
    allowed_protocols: &PhpArray,
) -> Result<Vec<u8>, BadProtocolOnceError> {
    once(content, allowed_protocols, 1)
}

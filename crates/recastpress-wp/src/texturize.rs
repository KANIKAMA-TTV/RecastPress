//! Fixed-English wptexturize text-token subset. The caller selects an empty
//! shortcode registry, unset cockney replacements, default translations and no
//! relevant filters (including `all`), with source statics reset/run=true.
//! Whole valid UTF-8 input without literal `<` is required. This is not HTML or
//! shortcode parsing, a default-filter title renderer, escaping, or safe HTML.

mod primes;

use crate::wp_is_valid_utf8_bytes;

const OPEN_SINGLE_FLAG: &[u8] = b"<!--osq-->";
const OPEN_DOUBLE_FLAG: &[u8] = b"<!--oq-->";
const APOS_FLAG: &[u8] = b"<!--apos-->";
const OPEN_SINGLE: &[u8] = b"&#8216;";
const CLOSE_SINGLE: &[u8] = b"&#8217;";
const OPEN_DOUBLE: &[u8] = b"&#8220;";
const CLOSE_DOUBLE: &[u8] = b"&#8221;";

/// Explicit host logical-byte allowances, with no defaults. Work bytes bound
/// each initial/intermediate owned buffer, not cumulative allocation or CPU work.
/// Inputs, simultaneously held buffers, node/Vec capacity, RSS and concurrency
/// are not measured by these limits. Final output is checked after all passes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TexturizeLimits {
    max_input_bytes: usize,
    max_work_bytes: usize,
    max_output_bytes: usize,
}

impl TexturizeLimits {
    pub fn new(max_input_bytes: usize, max_work_bytes: usize, max_output_bytes: usize) -> Self {
        Self {
            max_input_bytes,
            max_work_bytes,
            max_output_bytes,
        }
    }

    pub(crate) fn input_limit(self) -> usize {
        self.max_input_bytes
    }

    pub(crate) fn work_limit(self) -> usize {
        self.max_work_bytes
    }

    pub(crate) fn output_limit(self) -> usize {
        self.max_output_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TexturizeError {
    UnsupportedValue,
    InputLimit,
    WorkLimit,
    OutputLimit,
    AllocationFailed,
}

impl std::fmt::Display for TexturizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "unsupported texturization input",
            Self::InputLimit => "texturization input limit exceeded",
            Self::WorkLimit => "texturization work buffer limit exceeded",
            Self::OutputLimit => "texturization output limit exceeded",
            Self::AllocationFailed => "texturization allocation failed",
        })
    }
}

impl std::error::Error for TexturizeError {}

pub(super) struct WorkBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

fn work_length(current: usize, extra: usize, limit: usize) -> Result<usize, TexturizeError> {
    current
        .checked_add(extra)
        .filter(|length| *length <= limit)
        .ok_or(TexturizeError::WorkLimit)
}

fn reserve_work(bytes: &mut Vec<u8>, extra: usize) -> Result<(), TexturizeError> {
    bytes
        .try_reserve(extra)
        .map_err(|_| TexturizeError::AllocationFailed)
}

impl WorkBuffer {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    pub(super) fn push(&mut self, bytes: &[u8]) -> Result<(), TexturizeError> {
        work_length(self.bytes.len(), bytes.len(), self.limit)?;
        reserve_work(&mut self.bytes, bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(super) fn find(raw: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        Some(0)
    } else {
        raw.windows(needle.len()).position(|part| part == needle)
    }
}

pub(super) fn replace_all(
    raw: &[u8],
    needle: &[u8],
    replacement: &[u8],
    limit: usize,
) -> Result<Vec<u8>, TexturizeError> {
    if needle.is_empty() {
        return Err(TexturizeError::UnsupportedValue);
    }
    let mut output = WorkBuffer::new(limit);
    let mut start = 0;
    while let Some(relative) = find(&raw[start..], needle) {
        let at = start + relative;
        output.push(&raw[start..at])?;
        output.push(replacement)?;
        start = at + needle.len();
    }
    output.push(&raw[start..])?;
    Ok(output.finish())
}

// PCRE-C wp_spaces_regexp, not Rust Unicode whitespace or PHP trim's byte set.
pub(super) fn space_at(raw: &[u8], position: usize) -> bool {
    raw.get(position)
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n' | b'\t' | b' '))
        || raw
            .get(position..)
            .is_some_and(|tail| tail.starts_with(b"\xc2\xa0") || tail.starts_with(b"&nbsp;"))
}

pub(super) fn space_before(raw: &[u8], position: usize) -> bool {
    let prefix = &raw[..position];
    prefix
        .last()
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n' | b'\t' | b' '))
        || prefix.ends_with(b"\xc2\xa0")
        || prefix.ends_with(b"&nbsp;")
}

fn end_context(raw: &[u8], position: usize) -> bool {
    position == raw.len() || (raw.get(position..) == Some(b"\n"))
}

pub(super) fn closing_context(raw: &[u8], position: usize) -> bool {
    end_context(raw, position)
        || raw
            .get(position)
            .is_some_and(|byte| b".,:;!?)}-]".contains(byte))
        || raw
            .get(position..)
            .is_some_and(|tail| tail.starts_with(b"&gt;"))
        || space_at(raw, position)
}

fn rewrite(
    raw: &[u8],
    limit: usize,
    mut rule: impl FnMut(&[u8], usize, &mut WorkBuffer) -> Result<Option<usize>, TexturizeError>,
) -> Result<Vec<u8>, TexturizeError> {
    let mut output = WorkBuffer::new(limit);
    let mut position = 0;
    while position < raw.len() {
        if let Some(length) = rule(raw, position, &mut output)? {
            position += length;
        } else {
            output.push(&raw[position..position + 1])?;
            position += 1;
        }
    }
    Ok(output.finish())
}

fn quote_rules(mut raw: Vec<u8>, quote: u8, limit: usize) -> Result<Vec<u8>, TexturizeError> {
    if quote == b'\'' {
        // The two abbreviated-year closing-quotation patterns run first.
        for close in [b'\'', b'"'] {
            raw = rewrite(&raw, limit, |raw, at, output| {
                if raw[at] == b'\''
                    && raw.get(at + 1).is_some_and(u8::is_ascii_digit)
                    && raw.get(at + 2).is_some_and(u8::is_ascii_digit)
                    && raw.get(at + 3) == Some(&close)
                    && closing_context(raw, at + 4)
                {
                    output.push(APOS_FLAG)?;
                    output.push(&raw[at + 1..at + 3])?;
                    output.push(if close == b'\'' {
                        CLOSE_SINGLE
                    } else {
                        CLOSE_DOUBLE
                    })?;
                    Ok(Some(4))
                } else {
                    Ok(None)
                }
            })?;
        }
        raw = rewrite(&raw, limit, |raw, at, output| {
            if raw[at] != b'\''
                || !raw.get(at + 1).is_some_and(u8::is_ascii_digit)
                || !raw.get(at + 2).is_some_and(u8::is_ascii_digit)
            {
                return Ok(None);
            }
            let after = at + 3;
            let forbidden = raw.get(after).is_some_and(|byte| {
                *byte == b'%'
                    || byte.is_ascii_digit()
                    || (matches!(byte, b'.' | b',')
                        && raw.get(after + 1).is_some_and(u8::is_ascii_digit))
            });
            if end_context(raw, after) || !forbidden {
                output.push(APOS_FLAG)?;
                Ok(Some(1))
            } else {
                Ok(None)
            }
        })?;
    }
    // Quoted decimal numbers preceded by the start or wp_spaces_regexp.
    raw = rewrite(&raw, limit, |raw, at, output| {
        if raw[at] != quote
            || (at != 0 && !space_before(raw, at))
            || !raw.get(at + 1).is_some_and(u8::is_ascii_digit)
        {
            return Ok(None);
        }
        let mut end = at + 2;
        while raw
            .get(end)
            .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b','))
        {
            end += 1;
        }
        if raw.get(end) != Some(&quote) {
            return Ok(None);
        }
        output.push(if quote == b'\'' {
            OPEN_SINGLE_FLAG
        } else {
            OPEN_DOUBLE_FLAG
        })?;
        output.push(&raw[at + 1..end])?;
        output.push(if quote == b'\'' {
            CLOSE_SINGLE
        } else {
            CLOSE_DOUBLE
        })?;
        Ok(Some(end + 1 - at))
    })?;
    raw = rewrite(&raw, limit, |raw, at, output| {
        let prefix = &raw[..at];
        let prior = at == 0
            || space_before(raw, at)
            || prefix.ends_with(b"&lt;")
            || prefix.last().is_some_and(|byte| {
                if quote == b'\'' {
                    b"([{\"-".contains(byte)
                } else {
                    b"([{-".contains(byte)
                }
            });
        if raw[at] == quote && prior && (quote == b'\'' || !space_at(raw, at + 1)) {
            output.push(if quote == b'\'' {
                OPEN_SINGLE_FLAG
            } else {
                OPEN_DOUBLE_FLAG
            })?;
            Ok(Some(1))
        } else {
            Ok(None)
        }
    })?;
    if quote == b'\'' {
        raw = rewrite(&raw, limit, |raw, at, output| {
            let after = at + 1;
            let forbidden = end_context(raw, after)
                || space_at(raw, after)
                || raw
                    .get(after)
                    .is_some_and(|byte| b".,:;!?\"'(){}[]-".contains(byte))
                || raw
                    .get(after..)
                    .is_some_and(|tail| tail.starts_with(b"&lt;") || tail.starts_with(b"&gt;"));
            if raw[at] == b'\'' && !space_before(raw, at) && !forbidden {
                output.push(APOS_FLAG)?;
                Ok(Some(1))
            } else {
                Ok(None)
            }
        })?;
        raw = primes::texturize_primes(
            &raw,
            quote,
            b"&#8242;",
            OPEN_SINGLE_FLAG,
            CLOSE_SINGLE,
            limit,
        )?;
        raw = replace_all(&raw, APOS_FLAG, CLOSE_SINGLE, limit)?;
        replace_all(&raw, OPEN_SINGLE_FLAG, OPEN_SINGLE, limit)
    } else {
        raw = primes::texturize_primes(
            &raw,
            quote,
            b"&#8243;",
            OPEN_DOUBLE_FLAG,
            CLOSE_DOUBLE,
            limit,
        )?;
        replace_all(&raw, OPEN_DOUBLE_FLAG, OPEN_DOUBLE, limit)
    }
}

fn dash_rules(mut raw: Vec<u8>, limit: usize) -> Result<Vec<u8>, TexturizeError> {
    raw = replace_all(&raw, b"---", b"&#8212;", limit)?;
    raw = rewrite(&raw, limit, |raw, at, output| {
        if raw[at..].starts_with(b"--")
            && (at == 0 || space_before(raw, at))
            && (end_context(raw, at + 2) || space_at(raw, at + 2))
        {
            output.push(b"&#8212;")?;
            Ok(Some(2))
        } else {
            Ok(None)
        }
    })?;
    raw = rewrite(&raw, limit, |raw, at, output| {
        if raw[at..].starts_with(b"--") && !raw[..at].ends_with(b"xn") {
            output.push(b"&#8211;")?;
            Ok(Some(2))
        } else {
            Ok(None)
        }
    })?;
    rewrite(&raw, limit, |raw, at, output| {
        if raw[at] == b'-'
            && (at == 0 || space_before(raw, at))
            && (end_context(raw, at + 1) || space_at(raw, at + 1))
        {
            output.push(b"&#8211;")?;
            Ok(Some(1))
        } else {
            Ok(None)
        }
    })
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn boundary(raw: &[u8], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|before| raw.get(before))
        .is_some_and(|byte| word(*byte))
        != raw.get(at).is_some_and(|byte| word(*byte))
}

fn times_rule(raw: &[u8], limit: usize) -> Result<Vec<u8>, TexturizeError> {
    rewrite(raw, limit, |raw, at, output| {
        if !raw[at].is_ascii_digit() || !boundary(raw, at) {
            return Ok(None);
        }
        let mut middle = at + 1;
        while raw
            .get(middle)
            .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b','))
        {
            middle += 1;
        }
        if (raw[at] == b'0' && middle == at + 1)
            || raw.get(middle) != Some(&b'x')
            || !raw.get(middle + 1).is_some_and(u8::is_ascii_digit)
        {
            return Ok(None);
        }
        let mut end = middle + 2;
        while raw
            .get(end)
            .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b','))
        {
            end += 1;
        }
        while end > middle + 1 && !boundary(raw, end) {
            end -= 1;
        }
        if end <= middle + 1 {
            return Ok(None);
        }
        output.push(&raw[at..middle])?;
        output.push(b"&#215;")?;
        output.push(&raw[middle + 1..end])?;
        Ok(Some(end - at))
    })
}

fn entity_tail(raw: &[u8]) -> bool {
    let (digits, radix) = if let Some(tail) = raw.strip_prefix(b"#") {
        if matches!(tail.first(), Some(b'x' | b'X')) {
            (&tail[1..], 16)
        } else {
            (tail, 10)
        }
    } else {
        let count = raw
            .iter()
            .take_while(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'1'..=b'4'))
            .count();
        return (1..=8).contains(&count) && raw.get(count) == Some(&b';');
    };
    let count = digits
        .iter()
        .take_while(|byte| {
            if radix == 16 {
                byte.is_ascii_hexdigit()
            } else {
                byte.is_ascii_digit()
            }
        })
        .count();
    count > 0 && digits.get(count) == Some(&b';')
}

/// Fixed-profile return value only. Input cap precedes whole UTF-8/NoLiteralLt
/// admission; initial and every intermediate buffer obey the work-byte cap.
/// Final output cap is separate and errors never return a partial result.
/// Outputs contain HTML entity syntax and must not be passed to a literal-text
/// renderer as though they were a WordPress rendered HTML fragment.
pub fn wptexturize_default_english_no_html_bytes(
    raw: &[u8],
    limits: TexturizeLimits,
) -> Result<Vec<u8>, TexturizeError> {
    if raw.len() > limits.max_input_bytes {
        return Err(TexturizeError::InputLimit);
    }
    if raw.contains(&b'<') || !wp_is_valid_utf8_bytes(raw) {
        return Err(TexturizeError::UnsupportedValue);
    }
    let mut initial = WorkBuffer::new(limits.max_work_bytes);
    initial.push(raw)?;
    let mut result = initial.finish();
    let trim_only = raw
        .iter()
        .all(|byte| matches!(byte, 0 | b' ' | b'\t' | b'\n' | b'\r' | 0x0b));
    if raw != b"0" && !trim_only {
        for (from, to) in [
            (b"...".as_slice(), b"&#8230;".as_slice()),
            (b"``", OPEN_DOUBLE),
            (b"''", CLOSE_DOUBLE),
            (b" (tm)", b" &#8482;"),
            (b"'tain't", b"&#8217;tain&#8217;t"),
            (b"'twere", b"&#8217;twere"),
            (b"'twas", b"&#8217;twas"),
            (b"'tis", b"&#8217;tis"),
            (b"'twill", b"&#8217;twill"),
            (b"'til", b"&#8217;til"),
            (b"'bout", b"&#8217;bout"),
            (b"'nuff", b"&#8217;nuff"),
            (b"'round", b"&#8217;round"),
            (b"'cause", b"&#8217;cause"),
            (b"'em", b"&#8217;em"),
        ] {
            result = replace_all(&result, from, to, limits.max_work_bytes)?;
        }
        if result.contains(&b'\'') {
            result = quote_rules(result, b'\'', limits.max_work_bytes)?;
        }
        if result.contains(&b'"') {
            result = quote_rules(result, b'"', limits.max_work_bytes)?;
        }
        if result.contains(&b'-') {
            result = dash_rules(result, limits.max_work_bytes)?;
        }
        if result
            .windows(3)
            .any(|part| part[0].is_ascii_digit() && part[1] == b'x' && part[2].is_ascii_digit())
        {
            result = times_rule(&result, limits.max_work_bytes)?;
        }
        result = rewrite(&result, limits.max_work_bytes, |raw, at, output| {
            if raw[at] == b'&' && !entity_tail(&raw[at + 1..]) {
                output.push(b"&#038;")?;
                Ok(Some(1))
            } else {
                Ok(None)
            }
        })?;
    }
    if result.len() > limits.max_output_bytes {
        return Err(TexturizeError::OutputLimit);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_work_overflow_and_impossible_capacity_are_closed_without_large_inputs() {
        assert_eq!(
            work_length(usize::MAX, 1, usize::MAX),
            Err(TexturizeError::WorkLimit)
        );
        assert_eq!(work_length(3, 4, 7), Ok(7));
        assert_eq!(work_length(3, 4, 6), Err(TexturizeError::WorkLimit));
        let mut bytes = Vec::new();
        assert_eq!(
            reserve_work(&mut bytes, usize::MAX),
            Err(TexturizeError::AllocationFailed)
        );
        assert!(bytes.is_empty());
        let mut output = WorkBuffer::new(1);
        assert_eq!(output.push(b"too long"), Err(TexturizeError::WorkLimit));
        assert!(output.finish().is_empty());
    }
}

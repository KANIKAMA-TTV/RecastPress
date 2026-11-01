//! Fixed sanitize_title_with_dashes subset: no '<', post-strip UTF-8, no mbstring.
use crate::{Utf8UriEncodeError, utf8_uri_encode_utf8_bytes};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TitleDashContext {
    Display,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TitleDashError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for TitleDashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedValue => "title is outside the explicit title-dash subset",
            Self::OutputLimit => "title-dash output exceeds the host byte limit",
            Self::AllocationFailed => "title-dash allocation failed",
        })
    }
}
impl std::error::Error for TitleDashError {}
impl From<Utf8UriEncodeError> for TitleDashError {
    fn from(error: Utf8UriEncodeError) -> Self {
        match error {
            Utf8UriEncodeError::UnsupportedValue => Self::UnsupportedValue,
            Utf8UriEncodeError::OutputLimit => Self::OutputLimit,
            Utf8UriEncodeError::AllocationFailed => Self::AllocationFailed,
        }
    }
}

fn buffer(length: usize) -> Result<Vec<u8>, TitleDashError> {
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|_| TitleDashError::AllocationFailed)?;
    Ok(out)
}

fn octet(input: &[u8], index: usize) -> bool {
    input.len() - index >= 3
        && input[index] == b'%'
        && input[index + 1].is_ascii_hexdigit()
        && input[index + 2].is_ascii_hexdigit()
}

fn protect(input: &[u8]) -> Result<Vec<u8>, TitleDashError> {
    let mut length = input.len();
    let mut index = 0;
    while index < input.len() {
        if octet(input, index) {
            length = length
                .checked_add(5)
                .ok_or(TitleDashError::AllocationFailed)?;
            index += 3;
        } else {
            index += 1;
        }
    }
    let mut out = buffer(length)?;
    let mut index = 0;
    while index < input.len() {
        if octet(input, index) {
            out.extend_from_slice(b"---");
            out.extend_from_slice(&input[index + 1..index + 3]);
            out.extend_from_slice(b"---");
            index += 3;
        } else {
            out.push(input[index]);
            index += 1;
        }
    }
    Ok(out)
}

fn restore(input: &[u8]) -> Result<Vec<u8>, TitleDashError> {
    let mut out = buffer(input.len())?;
    let mut index = 0;
    while index < input.len() {
        // Source preg_replace also consumes pre-existing literal sentinels.
        if input.len() - index >= 8
            && &input[index..index + 3] == b"---"
            && input[index + 3].is_ascii_hexdigit()
            && input[index + 4].is_ascii_hexdigit()
            && &input[index + 5..index + 8] == b"---"
        {
            out.push(b'%');
            out.extend_from_slice(&input[index + 3..index + 5]);
            index += 8;
        } else {
            out.push(input[index]);
            index += 1;
        }
    }
    Ok(out)
}

// These are the ordered literals in fixed formatting.php:2302-2388.
const SAVE_DASH_OCTETS: &[&[u8]] = &[b"%c2%a0", b"%e2%80%91", b"%e2%80%93", b"%e2%80%94"];
const SAVE_DASH_ENTITIES: &[&[u8]] = &[
    b"&nbsp;", b"&#8209;", b"&#160;", b"&ndash;", b"&#8211;", b"&mdash;", b"&#8212;",
];
const SAVE_REMOVE: &[&[u8]] = &[
    b"%c2%ad",
    b"%c2%a1",
    b"%c2%bf",
    b"%c2%ab",
    b"%c2%bb",
    b"%e2%80%b9",
    b"%e2%80%ba",
    b"%e2%80%98",
    b"%e2%80%99",
    b"%e2%80%9c",
    b"%e2%80%9d",
    b"%e2%80%9a",
    b"%e2%80%9b",
    b"%e2%80%9e",
    b"%e2%80%9f",
    b"%e2%80%a2",
    b"%c2%a9",
    b"%c2%ae",
    b"%c2%b0",
    b"%e2%80%a6",
    b"%e2%84%a2",
    b"%c2%b4",
    b"%cb%8a",
    b"%cc%81",
    b"%cd%81",
    b"%cc%80",
    b"%cc%84",
    b"%cc%8c",
    b"%e2%80%8b",
    b"%e2%80%8c",
    b"%e2%80%8d",
    b"%e2%80%8e",
    b"%e2%80%8f",
    b"%e2%80%aa",
    b"%e2%80%ab",
    b"%e2%80%ac",
    b"%e2%80%ad",
    b"%e2%80%ae",
    b"%ef%bb%bf",
    b"%ef%bf%bc",
];
const SAVE_WIDTH_DASH: &[&[u8]] = &[
    b"%e2%80%80",
    b"%e2%80%81",
    b"%e2%80%82",
    b"%e2%80%83",
    b"%e2%80%84",
    b"%e2%80%85",
    b"%e2%80%86",
    b"%e2%80%87",
    b"%e2%80%88",
    b"%e2%80%89",
    b"%e2%80%8a",
    b"%e2%80%a8",
    b"%e2%80%a9",
    b"%e2%80%af",
];

fn replace(input: &[u8], needle: &[u8], replacement: &[u8]) -> Result<Vec<u8>, TitleDashError> {
    // All fixed replacements are nonexpanding, not recursive within one pass.
    let mut out = buffer(input.len())?;
    let mut index = 0;
    while index < input.len() {
        if input[index..].starts_with(needle) {
            out.extend_from_slice(replacement);
            index += needle.len();
        } else {
            out.push(input[index]);
            index += 1;
        }
    }
    Ok(out)
}

fn save_replacements(mut value: Vec<u8>) -> Result<Vec<u8>, TitleDashError> {
    for needle in SAVE_DASH_OCTETS {
        value = replace(&value, needle, b"-")?;
    }
    for needle in SAVE_DASH_ENTITIES {
        value = replace(&value, needle, b"-")?;
    }
    value = replace(&value, b"/", b"-")?;
    for needle in SAVE_REMOVE {
        value = replace(&value, needle, b"")?;
    }
    for needle in SAVE_WIDTH_DASH {
        value = replace(&value, needle, b"-")?;
    }
    replace(&value, b"%c3%97", b"x")
}

fn remove_entities(input: &[u8]) -> Result<Vec<u8>, TitleDashError> {
    let mut out = buffer(input.len())?;
    let mut index = 0;
    while index < input.len() {
        if input[index] != b'&' || input.len() - index < 2 || input[index + 1] == b'\n' {
            out.push(input[index]);
            index += 1;
            continue;
        }
        // /&.+?;/ needs at least one body byte; even ';' can be that byte.
        let mut end = index + 2;
        while end < input.len() && !matches!(input[end], b'\n' | b';') {
            end += 1;
        }
        if end < input.len() && input[end] == b';' {
            index = end + 1;
        } else {
            // No nested '&' in this failed suffix can match before LF/end.
            out.extend_from_slice(&input[index..end]);
            index = end;
        }
    }
    Ok(out)
}

fn final_walk(
    input: &[u8],
    mut emit: impl FnMut(u8) -> Result<(), TitleDashError>,
) -> Result<(), TitleDashError> {
    let mut have_content = false;
    let mut pending_dash = false;
    for &byte in input {
        if matches!(byte, b'.' | b' ' | b'-') {
            pending_dash = have_content;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'%' | b'_')
        {
            if pending_dash {
                emit(b'-')?;
                pending_dash = false;
            }
            emit(byte)?;
            have_content = true;
        }
        // Other bytes, including TAB/LF/CR/VT/FF, fail the source whitelist
        // before its whitespace regexp. Only ASCII space can become a dash.
    }
    Ok(())
}

/// Explicit NoLiteral<'<' raw / post-strip valid UTF-8 / no-mbstring subset.
/// Strip-tags in this subset deletes NUL and does not trim. No remove_accents,
/// hooks, locale access, uniqueness, route admission or DB write is performed.
/// The fixed 200-byte URI limit precedes cleanup; max_output caps final bytes
/// only, not intermediate allocations, RSS, input size or URL validity.
pub fn sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes(
    raw: &[u8],
    context: TitleDashContext,
    max_output: usize,
) -> Result<Vec<u8>, TitleDashError> {
    if raw.contains(&b'<') {
        return Err(TitleDashError::UnsupportedValue);
    }
    let mut stripped = buffer(raw.iter().filter(|&&byte| byte != 0).count())?;
    stripped.extend(raw.iter().copied().filter(|&byte| byte != 0));
    if std::str::from_utf8(&stripped).is_err() {
        return Err(TitleDashError::UnsupportedValue);
    }
    let mut protected = protect(&stripped)?;
    protected.retain(|&byte| byte != b'%');
    let restored = restore(&protected)?;
    let mut encoded = utf8_uri_encode_utf8_bytes(&restored, 200, false, usize::MAX)?;
    encoded.make_ascii_lowercase();
    if context == TitleDashContext::Save {
        encoded = save_replacements(encoded)?;
    }
    let cleaned = remove_entities(&encoded)?;
    let mut length = 0usize;
    final_walk(&cleaned, |_| {
        length = length
            .checked_add(1)
            .filter(|&length| length <= max_output)
            .ok_or(TitleDashError::OutputLimit)?;
        Ok(())
    })?;
    let mut out = buffer(length)?;
    final_walk(&cleaned, |byte| {
        out.push(byte);
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_replacement_literals_keep_order_and_nonexpanding_lengths() {
        assert_eq!(
            (
                SAVE_DASH_OCTETS.len(),
                SAVE_DASH_ENTITIES.len(),
                SAVE_REMOVE.len(),
                SAVE_WIDTH_DASH.len()
            ),
            (4, 7, 40, 14)
        );
        for group in [SAVE_DASH_OCTETS, SAVE_DASH_ENTITIES, SAVE_WIDTH_DASH] {
            for needle in group {
                let mut input = b"a".to_vec();
                input.extend_from_slice(needle);
                input.push(b'b');
                assert_eq!(save_replacements(input).unwrap(), b"a-b");
            }
        }
        for needle in SAVE_REMOVE {
            let mut input = b"a".to_vec();
            input.extend_from_slice(needle);
            input.push(b'b');
            assert_eq!(save_replacements(input).unwrap(), b"ab");
        }
        assert_eq!(save_replacements(b"a%c3%97b".to_vec()).unwrap(), b"axb");
    }

    #[test]
    fn intermediate_sentinels_are_nonrecursive_and_checked_allocation_is_fallible() {
        assert_eq!(protect(b"%4%30A").unwrap(), b"%4---30---A");
        assert_eq!(restore(b"---41------42---").unwrap(), b"%41%42");
        assert_eq!(buffer(usize::MAX), Err(TitleDashError::AllocationFailed));
        assert_eq!(remove_entities(b"a&;;b&x\n;c").unwrap(), b"ab&x\n;c");
    }
}

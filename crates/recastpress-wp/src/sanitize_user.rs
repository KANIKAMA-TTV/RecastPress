//! Explicit no-Intl/UTF-8/C-locale/empty-filter sanitize_user byte subset.
use crate::strip_all_tags::{buffer, charged, php_trim6};
use crate::{
    RemoveAccentsError, StripAllTagsError, remove_accents_utf8_explicit_locale_bytes,
    wp_strip_all_tags_no_open_angle_bytes,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SanitizeUserError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for SanitizeUserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "input or locale is outside the explicit username subset",
            Self::OutputLimit => "sanitized username exceeds the host byte limit",
            Self::AllocationFailed => "username preprocessing allocation failed",
        })
    }
}
impl std::error::Error for SanitizeUserError {}
impl From<StripAllTagsError> for SanitizeUserError {
    fn from(error: StripAllTagsError) -> Self {
        match error {
            StripAllTagsError::UnsupportedValue => Self::UnsupportedValue,
            StripAllTagsError::OutputLimit => Self::OutputLimit,
            StripAllTagsError::AllocationFailed => Self::AllocationFailed,
        }
    }
}
impl From<RemoveAccentsError> for SanitizeUserError {
    fn from(error: RemoveAccentsError) -> Self {
        match error {
            RemoveAccentsError::UnsupportedValue => Self::UnsupportedValue,
            RemoveAccentsError::OutputLimit => Self::OutputLimit,
            RemoveAccentsError::AllocationFailed => Self::AllocationFailed,
        }
    }
}

fn remove_percent(input: &[u8]) -> Result<Vec<u8>, SanitizeUserError> {
    let mut out = buffer(input.len())?;
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%'
            && input.len() - i >= 3
            && input[i + 1].is_ascii_hexdigit()
            && input[i + 2].is_ascii_hexdigit()
        {
            i += 3;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    Ok(out)
}

// /&.+?;/ without dotall: at least one non-LF body byte and the first eligible
// semicolon. Failed suffixes before LF/end contain no later successful start,
// so emit them as one span rather than rescanning a quadratic ampersand run.
fn entities(input: &[u8], mut emit: impl FnMut(&[u8])) {
    let mut i = 0;
    while i < input.len() {
        let Some(offset) = input[i..].iter().position(|b| *b == b'&') else {
            emit(&input[i..]);
            break;
        };
        let start = i + offset;
        emit(&input[i..start]);
        if input.len() - start < 2 || input[start + 1] == b'\n' {
            emit(&input[start..start + 1]);
            i = start + 1;
            continue;
        }
        let mut end = start + 2;
        while end < input.len() && input[end] != b'\n' && input[end] != b';' {
            end += 1;
        }
        if end < input.len() && input[end] == b';' {
            i = end + 1;
        } else {
            emit(&input[start..end]);
            i = end;
        }
    }
}
fn allowed(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'_' | b'.' | b'-' | b'@')
}
fn c_space(byte: u8) -> bool {
    matches!(byte, 9..=13 | 32)
}
fn walk_final(
    input: &[u8],
    mut emit: impl FnMut(&[u8]) -> Result<(), StripAllTagsError>,
) -> Result<(), StripAllTagsError> {
    let input = php_trim6(input);
    let mut in_space = false;
    for (i, byte) in input.iter().enumerate() {
        if c_space(*byte) {
            if !in_space {
                emit(b" ")?;
            }
            in_space = true;
        } else {
            emit(&input[i..i + 1])?;
            in_space = false;
        }
    }
    Ok(())
}

/// formatting.php:2149, explicit PHP-truthy locale Bytes, no Intl, C whitespace
/// and empty sanitize_user filter. Strip(false) precedes UTF-8 validation, so
/// NUL removal can repair a split UTF-8 sequence. Then literal accent mapping,
/// one %HH deletion pass, shortest non-LF entity removal, optional strict ASCII
/// class, trim6 and C whitespace collapse. No lowercase or recursive cleanup.
///
/// Only the final output cap applies. Unpooled strip/accent/percent/entity
/// intermediates must complete before it; input/intermediate/RSS/peak/capacity
/// and concurrency are not bounded by this cap. No callback/site/DB/auth policy.
pub fn sanitize_user_utf8_no_open_angle_empty_filters_bytes(
    raw: &[u8],
    strict: bool,
    locale: &[u8],
    cap: usize,
) -> Result<Vec<u8>, SanitizeUserError> {
    if locale.is_empty() || locale == b"0" {
        return Err(SanitizeUserError::UnsupportedValue);
    }
    let stripped = wp_strip_all_tags_no_open_angle_bytes(raw, false, usize::MAX)?;
    let accented = remove_accents_utf8_explicit_locale_bytes(&stripped, locale, usize::MAX)?;
    let percent = remove_percent(&accented)?;
    let mut filtered = buffer(percent.len())?;
    entities(&percent, |chunk| {
        if strict {
            filtered.extend(chunk.iter().copied().filter(|b| allowed(*b)));
        } else {
            filtered.extend_from_slice(chunk);
        }
    });
    let mut length = 0;
    walk_final(&filtered, |chunk| {
        length = charged(length, chunk.len(), cap)?;
        Ok(())
    })?;
    let mut out = buffer(length)?;
    walk_final(&filtered, |chunk| {
        out.extend_from_slice(chunk);
        Ok(())
    })?;
    debug_assert_eq!(out.len(), length);
    Ok(out)
}

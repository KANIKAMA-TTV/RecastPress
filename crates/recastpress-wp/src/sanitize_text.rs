//! Fixed UTF-8-site/no-mbstring sanitize_text_field/textarea return subset.
//! The caller selects no own-name or `all` filters. Only raw Bytes without a
//! literal `<` are supported, so the source's HTML branch is never entered.
//! Invalid UTF-8 yields empty bytes, not a replacement-decoded string. No HTML
//! safety, DB transformation, UI preprocessing or principal policy is implied.

use crate::wp_is_valid_utf8_bytes;
use std::fmt;

/// Explicit host logical caps, with no defaults. A percent round removes all
/// occurrences of the first matched exact three-byte token, not one occurrence.
/// The output cap applies only after all shrinking passes. Allocation capacity,
/// input ownership, peak/RSS/concurrency, shared pools and CPU time are not
/// measured. The algorithm may rescan the input on every bounded removal round.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SanitizeTextLimits {
    max_input_bytes: usize,
    max_output_bytes: usize,
    max_percent_rounds: usize,
}

impl SanitizeTextLimits {
    pub fn new(max_input_bytes: usize, max_output_bytes: usize, max_percent_rounds: usize) -> Self {
        Self {
            max_input_bytes,
            max_output_bytes,
            max_percent_rounds,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SanitizeTextError {
    UnsupportedValue,
    InputLimit,
    OutputLimit,
    PercentRoundLimit,
    AllocationFailed,
}

impl fmt::Display for SanitizeTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "unsupported text sanitization input",
            Self::InputLimit => "text sanitization input limit exceeded",
            Self::OutputLimit => "text sanitization output limit exceeded",
            Self::PercentRoundLimit => "text sanitization percent round limit exceeded",
            Self::AllocationFailed => "text sanitization allocation failed",
        })
    }
}

impl std::error::Error for SanitizeTextError {}

fn trim_byte(byte: u8) -> bool {
    matches!(byte, 0 | b' ' | b'\t' | b'\r' | b'\n' | 0x0b)
}

fn trim6(bytes: &mut Vec<u8>) {
    let start = bytes
        .iter()
        .position(|byte| !trim_byte(*byte))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !trim_byte(*byte))
        .map_or(start, |at| at + 1);
    bytes.copy_within(start..end, 0);
    bytes.truncate(end - start);
}

fn collapse(bytes: &mut Vec<u8>, selected: impl Fn(u8) -> bool) {
    let mut written = 0;
    let mut in_run = false;
    for read in 0..bytes.len() {
        let byte = bytes[read];
        if selected(byte) {
            if !in_run {
                bytes[written] = b' ';
                written += 1;
            }
            in_run = true;
        } else {
            bytes[written] = byte;
            written += 1;
            in_run = false;
        }
    }
    bytes.truncate(written);
}

fn first_percent(bytes: &[u8]) -> Option<[u8; 3]> {
    bytes
        .windows(3)
        .find(|part| part[0] == b'%' && part[1].is_ascii_hexdigit() && part[2].is_ascii_hexdigit())
        .map(|part| [part[0], part[1], part[2]])
}

fn remove_exact(bytes: &mut Vec<u8>, token: [u8; 3]) {
    // Snapshot the selected token: writes must never modify the pattern being
    // matched. The original read cursor only moves forward, so newly joined
    // redexes are not reconsidered until the next outer round.
    let mut read = 0;
    let mut written = 0;
    while read < bytes.len() {
        if bytes[read..].starts_with(&token) {
            read += token.len();
        } else {
            bytes[written] = bytes[read];
            read += 1;
            written += 1;
        }
    }
    bytes.truncate(written);
}

fn sanitize(
    raw: &[u8],
    keep_newlines: bool,
    limits: SanitizeTextLimits,
) -> Result<Vec<u8>, SanitizeTextError> {
    if raw.len() > limits.max_input_bytes {
        return Err(SanitizeTextError::InputLimit);
    }
    if raw.contains(&b'<') {
        return Err(SanitizeTextError::UnsupportedValue);
    }
    // Reuse the borrowed validator only. The existing wp_check_invalid_utf8
    // wrapper copies valid inputs infallibly, which is not this host contract.
    if !wp_is_valid_utf8_bytes(raw) {
        return Ok(Vec::new());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(raw.len())
        .map_err(|_| SanitizeTextError::AllocationFailed)?;
    bytes.extend_from_slice(raw);
    if !keep_newlines {
        collapse(&mut bytes, |byte| {
            matches!(byte, b'\r' | b'\n' | b'\t' | b' ')
        });
    }
    trim6(&mut bytes);
    let mut rounds = 0_usize;
    while let Some(token) = first_percent(&bytes) {
        if rounds >= limits.max_percent_rounds {
            return Err(SanitizeTextError::PercentRoundLimit);
        }
        rounds = rounds
            .checked_add(1)
            .ok_or(SanitizeTextError::PercentRoundLimit)?;
        remove_exact(&mut bytes, token);
    }
    if rounds != 0 {
        collapse(&mut bytes, |byte| byte == b' ');
        trim6(&mut bytes);
    }
    if bytes.len() > limits.max_output_bytes {
        return Err(SanitizeTextError::OutputLimit);
    }
    Ok(bytes)
}

/// NoLiteralLt raw Bytes, fixed UTF-8 site and no relevant filters. Input cap
/// precedes NoLt admission; invalid UTF-8 then yields empty without allocation.
/// Field mode collapses only CR/LF/TAB/SPACE before trim6, never all Unicode or
/// C-whitespace. The HTML/tag-stripping path is outside this profile: interior
/// NULs remain. Percent tokens are deleted, never URL-decoded.
pub fn sanitize_text_field_utf8_site_no_open_angle_empty_filters_bytes(
    raw: &[u8],
    limits: SanitizeTextLimits,
) -> Result<Vec<u8>, SanitizeTextError> {
    sanitize(raw, false, limits)
}

/// Same raw/site/filter domain as field mode, preserving internal newlines,
/// tabs and spaces on entry. Only when at least one percent removal occurred
/// does the final pass collapse ASCII SPACE runs and trim6 again. All work is
/// in one fallibly reserved input-sized buffer; no round allocates another.
pub fn sanitize_textarea_field_utf8_site_no_open_angle_empty_filters_bytes(
    raw: &[u8],
    limits: SanitizeTextLimits,
) -> Result<Vec<u8>, SanitizeTextError> {
    sanitize(raw, true, limits)
}

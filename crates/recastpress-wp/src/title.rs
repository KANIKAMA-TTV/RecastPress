//! Prepared-post get_the_title return subset using the fixed English default
//! the_title callbacks: priority 10 wptexturize/convert_chars/trim, then priority
//! 11 capital_P_dangit. The caller attests frontend, raw WP_Post context, empty
//! extra/relevant filters including `all`, default English translations, empty
//! shortcodes, unset cockney replacements, and initialized run=true statics.
//! Native composition does not execute hooks or maintain their bookkeeping.
//! No DB lookup, post selection, authorization or visibility decision is made.

use crate::texturize::{
    TexturizeError, TexturizeLimits, WorkBuffer, replace_all,
    wptexturize_default_english_no_html_bytes,
};

/// Borrowed raw title for one positive-ID publish/unprotected post. ID/status/
/// password are validated as profile inputs, not retained as an identity or
/// principal. Their association with the title is the caller's responsibility.
/// Title UTF-8/NoLiteralLt admission is deferred until after the getter's input
/// cap, so construction itself does not scan or copy an unbounded title.
pub struct PreparedTitlePost<'a> {
    raw_title: &'a [u8],
}

impl<'a> PreparedTitlePost<'a> {
    pub fn new(
        id: i64,
        raw_title: &'a [u8],
        password: &[u8],
        status: &[u8],
    ) -> Result<Self, TitleFormatError> {
        if id <= 0 || status != b"publish" || !(password.is_empty() || password == b"0") {
            return Err(TitleFormatError::UnsupportedValue);
        }
        Ok(Self { raw_title })
    }
}

impl std::fmt::Debug for PreparedTitlePost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedTitlePost([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TitleFormatError {
    UnsupportedValue,
    InputLimit,
    WorkLimit,
    OutputLimit,
    AllocationFailed,
}

impl std::fmt::Display for TitleFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "unsupported prepared title input",
            Self::InputLimit => "prepared title input limit exceeded",
            Self::WorkLimit => "prepared title work buffer limit exceeded",
            Self::OutputLimit => "prepared title output limit exceeded",
            Self::AllocationFailed => "prepared title allocation failed",
        })
    }
}

impl std::error::Error for TitleFormatError {}

fn format_error(error: TexturizeError) -> TitleFormatError {
    match error {
        TexturizeError::UnsupportedValue => TitleFormatError::UnsupportedValue,
        TexturizeError::InputLimit => TitleFormatError::InputLimit,
        TexturizeError::WorkLimit => TitleFormatError::WorkLimit,
        TexturizeError::OutputLimit => TitleFormatError::OutputLimit,
        TexturizeError::AllocationFailed => TitleFormatError::AllocationFailed,
    }
}

// convert_chars has a different pattern from texturize's ampersand guard:
// &([^#])(?![a-z1-4]{1,8};) /i. The first byte after & is consumed BEFORE the
// negative lookahead. A match consumes two original bytes, never reprocessing a
// captured ampersand. This is not HTML entity recognition or normalization.
fn convert_chars(raw: &[u8], work_limit: usize) -> Result<Vec<u8>, TexturizeError> {
    let mut output = WorkBuffer::new(work_limit);
    let mut at = 0;
    while at < raw.len() {
        if raw[at] == b'&' && raw.get(at + 1).is_some_and(|byte| *byte != b'#') {
            let remainder = &raw[at + 2..];
            let count = remainder
                .iter()
                .take_while(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'1'..=b'4'))
                .count();
            if !(1..=8).contains(&count) || remainder.get(count) != Some(&b';') {
                output.push(b"&#038;")?;
                output.push(&raw[at + 1..at + 2])?;
                at += 2;
                continue;
            }
        }
        output.push(&raw[at..at + 1])?;
        at += 1;
    }
    Ok(output.finish())
}

fn trim6(raw: &[u8]) -> &[u8] {
    let whitespace = |byte: &u8| matches!(byte, 0 | b' ' | b'\t' | b'\n' | b'\r' | 0x0b);
    let start = raw
        .iter()
        .position(|byte| !whitespace(byte))
        .unwrap_or(raw.len());
    let end = raw
        .iter()
        .rposition(|byte| !whitespace(byte))
        .map_or(start, |at| at + 1);
    &raw[start..end]
}

/// Fixed-profile get_the_title return bytes for one caller-prepared raw post.
/// Whole valid UTF-8 and no literal `<` are required, after the input limit.
/// The shared three explicit limits are input/per-buffer work/final output;
/// work includes all texturize intermediates and the later formatter buffers.
/// Final output is checked ONLY after convert_chars, trim6 and capitalP.
///
/// Resulting entity syntax is not SafeHtml, an authorization/visibility result,
/// or a literal PreparedPosts title. This function does not escape HTML; callers
/// must choose the correct later output context without double-escaping through
/// a literal renderer. Node/capacity/peak/RSS/CPU/shared budgets are not measured.
pub fn get_the_title_prepared_frontend_default_english_no_html_bytes(
    post: &PreparedTitlePost<'_>,
    limits: TexturizeLimits,
) -> Result<Vec<u8>, TitleFormatError> {
    let work_limit = limits.work_limit();
    let texturized = wptexturize_default_english_no_html_bytes(
        post.raw_title,
        TexturizeLimits::new(limits.input_limit(), work_limit, work_limit),
    )
    .map_err(format_error)?;
    let converted = convert_chars(&texturized, work_limit).map_err(format_error)?;
    // Source capital_P_dangit sees current_filter()='the_title': replace every
    // exact Wordpress occurrence, including those inside larger byte strings.
    let result = replace_all(trim6(&converted), b"Wordpress", b"WordPress", work_limit)
        .map_err(format_error)?;
    if result.len() > limits.output_limit() {
        return Err(TitleFormatError::OutputLimit);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_pattern_consumes_captured_byte_and_has_its_own_entity_like_range() {
        for (raw, expected) in [
            (
                b"&a; &ab; &abcdefghi; &abcdefghij;".as_slice(),
                b"&#038;a; &ab; &abcdefghi; &#038;abcdefghij;".as_slice(),
            ),
            (b"&&amp; &&a; && &", b"&&amp; &&#038;a; &#038;& &"),
            (
                b"&#bad; &#x; &1; &14; &a0;",
                b"&#bad; &#x; &#038;1; &14; &#038;a0;",
            ),
            (b"&\n; &\0;", b"&#038;\n; &#038;\0;"),
        ] {
            assert_eq!(convert_chars(raw, 4096).unwrap(), expected);
        }
    }
}

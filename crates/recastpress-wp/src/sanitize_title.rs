//! Explicit UTF-8/locale, no-Intl/no-mbstring sanitize_title return-value subset.
//! Only the fixed default title-dash callback is modeled, without hook state.

use crate::{
    RemoveAccentsError, TitleDashContext, TitleDashError,
    remove_accents_utf8_explicit_locale_bytes,
    sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes, wp_is_valid_utf8_bytes,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SanitizeTitleContext {
    Save,
    Display,
    Query,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SanitizeTitleError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for SanitizeTitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "value is outside the explicit sanitize-title subset",
            Self::OutputLimit => "sanitized title exceeds the host byte limit",
            Self::AllocationFailed => "sanitized title allocation failed",
        })
    }
}
impl std::error::Error for SanitizeTitleError {}
impl From<RemoveAccentsError> for SanitizeTitleError {
    fn from(error: RemoveAccentsError) -> Self {
        match error {
            RemoveAccentsError::UnsupportedValue => Self::UnsupportedValue,
            RemoveAccentsError::OutputLimit => Self::OutputLimit,
            RemoveAccentsError::AllocationFailed => Self::AllocationFailed,
        }
    }
}
impl From<TitleDashError> for SanitizeTitleError {
    fn from(error: TitleDashError) -> Self {
        match error {
            TitleDashError::UnsupportedValue => Self::UnsupportedValue,
            TitleDashError::OutputLimit => Self::OutputLimit,
            TitleDashError::AllocationFailed => Self::AllocationFailed,
        }
    }
}

fn copy_bytes(value: &[u8]) -> Result<Vec<u8>, SanitizeTitleError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| SanitizeTitleError::AllocationFailed)?;
    result.extend_from_slice(value);
    Ok(result)
}

fn select_final(
    candidate: Vec<u8>,
    fallback: &[u8],
    maximum: usize,
) -> Result<Vec<u8>, SanitizeTitleError> {
    let selected = if candidate.is_empty() {
        fallback
    } else {
        &candidate
    };
    if selected.len() > maximum {
        return Err(SanitizeTitleError::OutputLimit);
    }
    if candidate.is_empty() {
        copy_bytes(fallback)
    } else {
        Ok(candidate)
    }
}

/// Pinned formatting.php:2228 with one default callback (default-filters.php:309).
/// Raw title must have no literal '<', be wholly valid UTF-8, and locale must
/// be nonempty and not '0', uniformly for all three typed contexts. No Intl or
/// mbstring behavior, get_locale, generic filters or bookkeeping is modeled.
///
/// Save removes accents before title-dash processing; Display/Query do not.
/// The original raw_title predates accent removal but the fixed callback does
/// not use it. Only an empty callback result selects fallback; '0' stays '0'.
/// Fallback is arbitrary Bytes, copied unchanged, without input-domain guards.
/// Errors remain errors, never fallback. The cap applies only after selection,
/// not to intermediate data, capacity/RSS/peak/shared budgets or concurrency.
/// This is not URL/HTML safety, slug uniqueness, posting or publication policy.
pub fn sanitize_title_utf8_default_callback_explicit_locale_bytes(
    raw: &[u8],
    fallback: &[u8],
    context: SanitizeTitleContext,
    locale: &[u8],
    max_output: usize,
) -> Result<Vec<u8>, SanitizeTitleError> {
    if raw.contains(&b'<') || !wp_is_valid_utf8_bytes(raw) || locale.is_empty() || locale == b"0" {
        return Err(SanitizeTitleError::UnsupportedValue);
    }
    let candidate = match context {
        SanitizeTitleContext::Save => {
            let accents = remove_accents_utf8_explicit_locale_bytes(raw, locale, usize::MAX)?;
            sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes(
                &accents,
                TitleDashContext::Save,
                usize::MAX,
            )?
        }
        SanitizeTitleContext::Display | SanitizeTitleContext::Query => {
            sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes(
                raw,
                TitleDashContext::Display,
                usize::MAX,
            )?
        }
    };
    select_final(candidate, fallback, max_output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_candidate_is_reused_and_helper_errors_keep_their_classification() {
        let candidate = b"candidate".to_vec();
        let address = candidate.as_ptr();
        let result = select_final(candidate, b"fallback", 9).unwrap();
        assert_eq!(result.as_ptr(), address);
        assert_eq!(
            SanitizeTitleError::from(RemoveAccentsError::AllocationFailed),
            SanitizeTitleError::AllocationFailed
        );
        assert_eq!(
            SanitizeTitleError::from(TitleDashError::OutputLimit),
            SanitizeTitleError::OutputLimit
        );
    }
}

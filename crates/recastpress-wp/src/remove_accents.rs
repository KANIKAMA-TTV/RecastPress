//! Fixed remove_accents byte-return subset: valid UTF-8, explicit locale,
//! and neither Intl normalizer function present in the reference process.
//! Pinned literal mappings only; no NFC/NFKD or Latin-1 fallback/coercion.

use crate::{remove_accents_table, wp_is_valid_utf8_bytes};
use std::fmt;

type Mapping = &'static [(&'static [u8], &'static [u8])];
// Pinned single-scalar base keys and the four-byte Catalan l-middle-dot-l.
// The private table contract rejects future data that violates this bound.
const MAX_KEY_BYTES: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoveAccentsError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for RemoveAccentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "value or locale is outside the explicit UTF-8 subset",
            Self::OutputLimit => "accent-removal output exceeds the host byte limit",
            Self::AllocationFailed => "accent-removal output allocation failed",
        })
    }
}
impl std::error::Error for RemoveAccentsError {}

fn locale_overrides(locale: &[u8]) -> Mapping {
    if locale.starts_with(b"de") {
        remove_accents_table::GERMAN
    } else if locale == b"da_DK" {
        remove_accents_table::DANISH
    } else if locale == b"ca" {
        remove_accents_table::CATALAN
    } else if locale == b"sr_RS" || locale == b"bs_BA" {
        remove_accents_table::SERBIAN_BOSNIAN
    } else {
        &[]
    }
}

fn lookup(table: Mapping, key: &[u8]) -> Option<&'static [u8]> {
    table
        .binary_search_by(|(candidate, _)| candidate.cmp(&key))
        .ok()
        .map(|index| table[index].1)
}

fn walk(
    input: &[u8],
    overrides: Mapping,
    mut emit: impl FnMut(&[u8]) -> Result<(), RemoveAccentsError>,
) -> Result<(), RemoveAccentsError> {
    let mut remaining = input;
    while !remaining.is_empty() {
        let mut matched = None;
        for length in (1..=MAX_KEY_BYTES.min(remaining.len())).rev() {
            let key = &remaining[..length];
            if let Some(replacement) =
                lookup(overrides, key).or_else(|| lookup(remove_accents_table::BASE, key))
            {
                matched = Some((length, replacement));
                break;
            }
        }
        if let Some((length, replacement)) = matched {
            emit(replacement)?;
            remaining = &remaining[length..];
        } else {
            // Unmapped bytes are copied exactly. This also preserves every
            // byte of an unmapped scalar/combining sequence without decoding.
            emit(&remaining[..1])?;
            remaining = &remaining[1..];
        }
    }
    Ok(())
}

fn charged(current: usize, extra: usize, maximum: usize) -> Result<usize, RemoveAccentsError> {
    current
        .checked_add(extra)
        .filter(|total| *total <= maximum)
        .ok_or(RemoveAccentsError::OutputLimit)
}
fn output_buffer(length: usize) -> Result<Vec<u8>, RemoveAccentsError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| RemoveAccentsError::AllocationFailed)?;
    Ok(output)
}

/// Fixed WP formatting.php:1611, explicit nonempty/non-'0' locale Bytes,
/// valid UTF-8 and no Intl normalization functions in the reference profile.
/// Locale matching is raw/case-sensitive: every 'de' prefix uses German rules;
/// da_DK, ca, sr_RS and bs_BA are exact. Other locale Bytes use the base table.
/// Empty/'0' locales and invalid UTF-8 return UnsupportedValue before the cap.
///
/// Longest source key wins, with locale values overriding base entries, and
/// emitted replacements are not processed again. NFD combining sequences stay
/// unchanged unless their original bytes match a pinned key; no NFC is run.
/// A shared count/emit walker checks final length before fallible allocation.
/// Deletions can fit cap0 regardless of input length. No partial result returns.
///
/// The final output cap is internal host policy, not an input/RSS/allocator
/// capacity/peak/shared-pool/concurrency bound or product manual setting.
/// No get_locale/site policy, Latin-1 fallback, casts, escaping, transliteration
/// beyond the literal dictionary, DB transformation or HTTP integration.
pub fn remove_accents_utf8_explicit_locale_bytes(
    value: &[u8],
    locale: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, RemoveAccentsError> {
    if locale.is_empty() || locale == b"0" || !wp_is_valid_utf8_bytes(value) {
        return Err(RemoveAccentsError::UnsupportedValue);
    }
    let overrides = locale_overrides(locale);
    let mut length = 0;
    walk(value, overrides, |chunk| {
        length = charged(length, chunk.len(), max_output_bytes)?;
        Ok(())
    })?;
    let mut output = output_buffer(length)?;
    walk(value, overrides, |chunk| {
        output.extend_from_slice(chunk);
        Ok(())
    })?;
    debug_assert_eq!(output.len(), length);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_tables_have_unique_sorted_valid_utf8_keys_within_the_longest_match_bound() {
        for (table, count) in [
            (remove_accents_table::BASE, 311),
            (remove_accents_table::GERMAN, 8),
            (remove_accents_table::DANISH, 6),
            (remove_accents_table::CATALAN, 1),
            (remove_accents_table::SERBIAN_BOSNIAN, 2),
        ] {
            assert_eq!(table.len(), count);
            for (key, value) in table {
                assert!(!key.is_empty() && key.len() <= MAX_KEY_BYTES);
                assert!(wp_is_valid_utf8_bytes(key));
                assert!(value.is_ascii());
            }
            assert!(table.windows(2).all(|pair| pair[0].0 < pair[1].0));
        }
        // The one multiscalar source key is necessary for PHP strtr semantics.
        assert_eq!(
            remove_accents_table::CATALAN,
            &[("l·l".as_bytes(), b"ll".as_slice())]
        );
    }

    #[test]
    fn checked_final_measure_and_fallible_capacity_overflow_are_distinct_without_allocation() {
        assert_eq!(charged(0, 0, 0), Ok(0));
        assert_eq!(charged(usize::MAX, 0, usize::MAX), Ok(usize::MAX));
        assert_eq!(
            charged(usize::MAX, 1, usize::MAX),
            Err(RemoveAccentsError::OutputLimit)
        );
        assert_eq!(charged(2, 1, 2), Err(RemoveAccentsError::OutputLimit));
        assert_eq!(
            output_buffer(usize::MAX),
            Err(RemoveAccentsError::AllocationFailed)
        );
    }
}

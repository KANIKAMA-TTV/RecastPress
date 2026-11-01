//! wp_strip_all_tags Bytes subset with no literal '<', not an HTML parser.
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StripAllTagsError {
    UnsupportedValue,
    OutputLimit,
    AllocationFailed,
}
impl fmt::Display for StripAllTagsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "input is outside the no-open-angle byte subset",
            Self::OutputLimit => "stripped output exceeds the host byte limit",
            Self::AllocationFailed => "stripped output allocation failed",
        })
    }
}
impl std::error::Error for StripAllTagsError {}

pub(crate) fn php_trim6(input: &[u8]) -> &[u8] {
    let trimmed = |byte: &u8| matches!(*byte, 0 | 9 | 10 | 11 | 13 | 32);
    let start = input
        .iter()
        .position(|b| !trimmed(b))
        .unwrap_or(input.len());
    let rest = &input[start..];
    let end = rest.iter().rposition(|b| !trimmed(b)).map_or(0, |i| i + 1);
    &rest[..end]
}
pub(crate) fn charged(
    current: usize,
    extra: usize,
    cap: usize,
) -> Result<usize, StripAllTagsError> {
    current
        .checked_add(extra)
        .filter(|sum| *sum <= cap)
        .ok_or(StripAllTagsError::OutputLimit)
}
pub(crate) fn buffer(length: usize) -> Result<Vec<u8>, StripAllTagsError> {
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|_| StripAllTagsError::AllocationFailed)?;
    Ok(out)
}

fn walk(
    input: &[u8],
    remove_breaks: bool,
    mut emit: impl FnMut(&[u8]) -> Result<(), StripAllTagsError>,
) -> Result<(), StripAllTagsError> {
    // Fuse the final trim by selecting original non-trim boundaries. Removing
    // NUL cannot change these boundaries; every collapsible byte is trim6.
    // Interior NUL removal still precedes run recognition and joins such runs.
    let input = php_trim6(input);
    let mut in_break = false;
    for (index, byte) in input.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        if remove_breaks && matches!(*byte, 9 | 10 | 13 | 32) {
            if !in_break {
                emit(b" ")?;
            }
            in_break = true;
        } else {
            emit(&input[index..index + 1])?;
            in_break = false;
        }
    }
    Ok(())
}

/// Fixed formatting.php:5600 with strict bool and Bytes lacking literal '<'.
/// Script/style removal is a no-op. Remove every NUL, optionally collapse only
/// CR/LF/TAB/SPACE runs, then PHP trim6. Invalid UTF-8, '>' and entities stay raw.
/// Other HTML/coercion/null domains are unsupported, not parsed or sanitized.
/// Shared count/emit traversal checks final output before fallible allocation.
/// Empty final output fits cap0 regardless of input length. No escaping/XSS
/// safety, RSS/peak/input/capacity/shared-pool or product-policy guarantee.
pub fn wp_strip_all_tags_no_open_angle_bytes(
    raw: &[u8],
    remove_breaks: bool,
    cap: usize,
) -> Result<Vec<u8>, StripAllTagsError> {
    if raw.contains(&b'<') {
        return Err(StripAllTagsError::UnsupportedValue);
    }
    let mut length = 0;
    walk(raw, remove_breaks, |chunk| {
        length = charged(length, chunk.len(), cap)?;
        Ok(())
    })?;
    let mut out = buffer(length)?;
    walk(raw, remove_breaks, |chunk| {
        out.extend_from_slice(chunk);
        Ok(())
    })?;
    debug_assert_eq!(out.len(), length);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_count_overflow_and_capacity_failure_remain_finite_classifications() {
        assert_eq!(
            charged(usize::MAX, 1, usize::MAX),
            Err(StripAllTagsError::OutputLimit)
        );
        assert_eq!(charged(0, 0, 0), Ok(0));
        assert_eq!(buffer(usize::MAX), Err(StripAllTagsError::AllocationFailed));
    }
}

//! Fixed-English byte translation of formatting.php:320–376. Each replacement
//! pass observes its own input; replacements are not recursively reconsidered.

use super::{TexturizeError, WorkBuffer, closing_context, find, replace_all};

const FLAG: &[u8] = b"<!--wp-prime-or-quote-->";

fn digit_before(raw: &[u8], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|previous| raw.get(previous))
        .is_some_and(u8::is_ascii_digit)
}

fn replace_if(
    raw: &[u8],
    needle: &[u8],
    replacement: &[u8],
    limit: usize,
    predicate: impl Fn(&[u8], usize) -> bool,
) -> Result<(Vec<u8>, usize), TexturizeError> {
    let mut output = WorkBuffer::new(limit);
    let mut copied = 0;
    let mut searched = 0;
    let mut count = 0_usize;
    while let Some(relative) = find(&raw[searched..], needle) {
        let at = searched + relative;
        if predicate(raw, at) {
            output.push(&raw[copied..at])?;
            output.push(replacement)?;
            copied = at + needle.len();
            count = count.checked_add(1).ok_or(TexturizeError::WorkLimit)?;
        }
        searched = at + needle.len();
    }
    output.push(&raw[copied..])?;
    Ok((output.finish(), count))
}

fn primes(raw: &[u8], needle: u8, prime: &[u8], limit: usize) -> Result<Vec<u8>, TexturizeError> {
    Ok(replace_if(raw, &[needle], prime, limit, digit_before)?.0)
}

fn sentence(
    raw: &[u8],
    after_open: bool,
    needle: u8,
    prime: &[u8],
    close_quote: &[u8],
    limit: usize,
) -> Result<Vec<u8>, TexturizeError> {
    let mut result;
    if after_open && find(raw, close_quote).is_none() {
        let (flagged, count) = replace_if(raw, &[needle], FLAG, limit, |bytes, at| {
            closing_context(bytes, at + 1)
        })?;
        if count > 1 {
            let (mut flagged, non_digit_count) =
                replace_if(&flagged, FLAG, close_quote, limit, |bytes, at| {
                    !digit_before(bytes, at)
                })?;
            if non_digit_count == 0 {
                // Source prefers the rightmost flag-period, even when a later
                // closing candidate exists. Otherwise it chooses the last flag.
                let at = flagged
                    .windows(FLAG.len() + 1)
                    .rposition(|part| part.starts_with(FLAG) && part[FLAG.len()] == b'.')
                    .or_else(|| flagged.windows(FLAG.len()).rposition(|part| part == FLAG))
                    .ok_or(TexturizeError::UnsupportedValue)?;
                let mut replaced = WorkBuffer::new(limit);
                replaced.push(&flagged[..at])?;
                replaced.push(close_quote)?;
                replaced.push(&flagged[at + FLAG.len()..])?;
                flagged = replaced.finish();
            }
            result = primes(&flagged, needle, prime, limit)?;
            result = replace_if(&result, FLAG, prime, limit, digit_before)?.0;
            result = replace_all(&result, FLAG, close_quote, limit)?;
        } else if count == 1 {
            result = replace_all(&flagged, FLAG, close_quote, limit)?;
            result = primes(&result, needle, prime, limit)?;
        } else {
            result = primes(&flagged, needle, prime, limit)?;
        }
    } else {
        result = primes(raw, needle, prime, limit)?;
        result = replace_if(&result, &[needle], close_quote, limit, |bytes, at| {
            closing_context(bytes, at + 1)
        })?
        .0;
    }
    if needle == b'"' && result.contains(&needle) {
        result = replace_all(&result, &[needle], close_quote, limit)?;
    }
    Ok(result)
}

pub(super) fn texturize_primes(
    raw: &[u8],
    needle: u8,
    prime: &[u8],
    open_quote: &[u8],
    close_quote: &[u8],
    max_work_bytes: usize,
) -> Result<Vec<u8>, TexturizeError> {
    // The parent supplies fixed, nonempty English flags/entities. Refuse invalid
    // private calls rather than allowing an empty-token scan to stop advancing.
    if !matches!(needle, b'\'' | b'"') || open_quote.is_empty() || close_quote.is_empty() {
        return Err(TexturizeError::UnsupportedValue);
    }
    let mut output = WorkBuffer::new(max_work_bytes);
    let mut start = 0;
    let mut after_open = false;
    loop {
        let split = find(&raw[start..], open_quote).map(|relative| start + relative);
        let end = split.unwrap_or(raw.len());
        let part = &raw[start..end];
        if part.contains(&needle) {
            output.push(&sentence(
                part,
                after_open,
                needle,
                prime,
                close_quote,
                max_work_bytes,
            )?)?;
        } else {
            // PHP's early continue also bypasses its final double-quote pass.
            output.push(part)?;
        }
        match split {
            Some(at) => {
                output.push(open_quote)?;
                start = at + open_quote.len();
                after_open = true;
            }
            None => return Ok(output.finish()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPEN: &[u8] = b"<!--oq-->";
    const CLOSE: &[u8] = b"&#8221;";
    const PRIME: &[u8] = b"&#8243;";

    fn double(raw: &[u8]) -> Vec<u8> {
        texturize_primes(raw, b'"', PRIME, OPEN, CLOSE, 4096).unwrap()
    }

    #[test]
    fn closing_candidates_choose_non_digit_then_rightmost_period_then_rightmost() {
        assert_eq!(double(b"<!--oq-->7\" 8\"."), b"<!--oq-->7&#8243; 8&#8221;.");
        assert_eq!(
            double(b"<!--oq-->7\". 8\"!"),
            b"<!--oq-->7&#8221;. 8&#8243;!"
        );
        assert_eq!(double(b"<!--oq-->7\" 8\"!"), b"<!--oq-->7&#8243; 8&#8221;!");
        assert_eq!(
            double(b"<!--oq-->7\" word\"."),
            b"<!--oq-->7&#8243; word&#8221;."
        );
    }

    #[test]
    fn a_single_closing_candidate_beats_prime_but_existing_close_changes_branch() {
        assert_eq!(double(b"<!--oq-->7\"."), b"<!--oq-->7&#8221;.");
        assert_eq!(
            double(b"<!--oq-->&#8221;7\"."),
            b"<!--oq-->&#8221;7&#8243;."
        );
        assert_eq!(double(b"7\"."), b"7&#8243;.");
        assert_eq!(double(b"<!--oq-->7\"x"), b"<!--oq-->7&#8243;x");
    }

    #[test]
    fn sentence_boundaries_final_lf_entities_and_double_leftovers_keep_source_order() {
        assert_eq!(double(b"<!--oq-->x\"\n"), b"<!--oq-->x&#8221;\n");
        assert_eq!(double(b"<!--oq-->x\"&gt;"), b"<!--oq-->x&#8221;&gt;");
        assert_eq!(
            double(b"a\"b<!--oq--><!--oq-->z\""),
            b"a&#8221;b<!--oq--><!--oq-->z&#8221;"
        );
        assert_eq!(
            texturize_primes(
                b"a'b 7'x",
                b'\'',
                b"&#8242;",
                b"<!--osq-->",
                b"&#8217;",
                100
            )
            .unwrap(),
            b"a'b 7&#8242;x"
        );
    }

    #[test]
    fn work_cap_covers_temporary_flags_even_when_the_final_result_would_fit() {
        let raw = b"<!--oq-->7\".";
        assert_eq!(
            texturize_primes(raw, b'"', PRIME, OPEN, CLOSE, 20),
            Err(TexturizeError::WorkLimit)
        );
        assert_eq!(double(raw), b"<!--oq-->7&#8221;.");
        assert_eq!(
            texturize_primes(b"unchanged", b'"', PRIME, OPEN, CLOSE, 8),
            Err(TexturizeError::WorkLimit)
        );
        assert_eq!(raw, b"<!--oq-->7\".");
    }
}

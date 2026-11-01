//! Explicit C-byte outer protocol pass, without URL authorization policy.

use crate::{
    BadProtocolOnceError, SlashZeroMode,
    bad_protocol_once::working_default,
    value::{PhpArray, PhpValue},
    wp_kses_no_null_bytes,
};
use std::fmt;

const TOP_INPUT_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadProtocolOuterError {
    UnsupportedProtocolValue,
    UnsupportedEntityValue,
    InputLimit,
    OutputLimit,
}

impl fmt::Display for BadProtocolOuterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedProtocolValue => "protocol allowlist contains a non-byte value",
            Self::UnsupportedEntityValue => {
                "entity decoder recognizes only values from 0 through 255"
            }
            Self::InputLimit => "protocol outer input exceeds the top-level host byte bound",
            Self::OutputLimit => "protocol outer exceeds the host output byte or allocation bound",
        })
    }
}

impl std::error::Error for BadProtocolOuterError {}

fn fastpath(content: &[u8], protocols: &PhpArray) -> bool {
    let protocol = if content.starts_with(b"https://") {
        b"https".as_slice()
    } else if content.starts_with(b"http://") {
        b"http".as_slice()
    } else {
        return false;
    };
    protocols
        .iter()
        .any(|(_, value)| matches!(value, PhpValue::Bytes(value) if value == protocol))
}

fn final_output(output: Vec<u8>, cap: usize) -> Result<Vec<u8>, BadProtocolOuterError> {
    if output.len() > cap {
        Err(BadProtocolOuterError::OutputLimit)
    } else {
        Ok(output)
    }
}

/// WordPress 7.1.3 `kses.php:1984` for Bytes, an explicit array with every
/// protocol value Bytes, and C locale. All values are prevalidated even if the
/// source fastpath or a skipped prefix would not inspect them; coercion and
/// non-byte diagnostic order are outside this subset. No default list is chosen.
///
/// Check the new 1MiB raw-input host bound only at entry. Apply global no_null
/// Remove once, then an exact lowercase http(s):// + strict lowercase allowlist
/// fastpath only at entry. Otherwise call the guard-free once working engine,
/// count=1 each time, at most six times. Strict byte equality returns a stable
/// result, even on pass six; a changed sixth result is discarded for empty.
/// No entity domain is imposed on unvisited prefixes/body or after loop cutoff.
///
/// Working/repair/recursive Vecs are allocated before the final cap and unpooled;
/// expanded loop data must not use the public once API's top-input guard. Final
/// cap checking occurs only on the completed fastpath/stable/empty result. These
/// bounds and allocation failures are not PHP semantics, product capacity policy
/// or shared RAM/RSS accounting. PCRE failures, other locales, URL grammar,
/// authorization, output safety and public HTTP/DB integration are unmodeled.
pub fn wp_kses_bad_protocol_c_bytes(
    content: &[u8],
    allowed_protocols: &PhpArray,
    max_output_bytes: usize,
) -> Result<Vec<u8>, BadProtocolOuterError> {
    if allowed_protocols
        .iter()
        .any(|(_, value)| !matches!(value, PhpValue::Bytes(_)))
    {
        return Err(BadProtocolOuterError::UnsupportedProtocolValue);
    }
    if content.len() > TOP_INPUT_LIMIT {
        return Err(BadProtocolOuterError::InputLimit);
    }
    let mut current = wp_kses_no_null_bytes(content, SlashZeroMode::Remove, content.len())
        .map_err(|_| BadProtocolOuterError::OutputLimit)?;
    if fastpath(&current, allowed_protocols) {
        return final_output(current, max_output_bytes);
    }
    for _ in 0..6 {
        let next = working_default(&current, allowed_protocols).map_err(|error| match error {
            BadProtocolOnceError::UnsupportedProtocolValue => {
                BadProtocolOuterError::UnsupportedProtocolValue
            }
            BadProtocolOnceError::UnsupportedEntityValue => {
                BadProtocolOuterError::UnsupportedEntityValue
            }
            BadProtocolOnceError::InputLimit => BadProtocolOuterError::InputLimit,
            BadProtocolOnceError::OutputLimit => BadProtocolOuterError::OutputLimit,
        })?;
        if next == current {
            return final_output(next, max_output_bytes);
        }
        current = next;
    }
    Ok(Vec::new())
}

//! Explicit C-byte subset of WordPress's once2 protocol helper.

use crate::{
    EntityDecodeError, SlashZeroMode,
    value::{PhpArray, PhpValue},
    wp_kses_decode_entities_byte_values, wp_kses_no_null_bytes,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadProtocolError {
    UnsupportedProtocolValue,
    UnsupportedEntityValue,
    InputLimit,
    OutputLimit,
}

impl fmt::Display for BadProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedProtocolValue => "protocol allowlist contains a non-byte value",
            Self::UnsupportedEntityValue => {
                "entity decoder recognizes only values from 0 through 255"
            }
            Self::InputLimit => "entity decoder input exceeds the host input byte bound",
            Self::OutputLimit => "protocol normalization exceeds the host byte or allocation bound",
        })
    }
}

impl std::error::Error for BadProtocolError {}

/// WordPress 7.1.3 `kses.php:2137` for a byte scheme, a PhpArray whose every
/// value is Bytes, and the C locale. Array keys are ignored. All protocol values
/// are validated, including values that PHP's array_any would short-circuit past;
/// non-byte coercions/diagnostic order are outside this explicit subset.
///
/// Preserve decode -> C whitespace removal -> no_null Remove -> ASCII lowercase
/// -> allowlist comparison. Protocol values undergo only ASCII case comparison,
/// not whitespace/entity/control normalization. This is not URL scheme grammar:
/// empty, "0", existing colons and arbitrary bytes can match and receive a colon.
/// Even an empty allowlist does not skip scheme decode/domain validation.
///
/// The existing decoder's 0..255 recognized-value domain and 1MiB input bound
/// apply. Intermediate Vecs/input/allowlist are outside the final-byte cap and
/// unpooled. Internal shrinking passes use their input lengths as working
/// allowances, never the caller's final cap. Only a match's scheme plus colon is
/// checked against that cap; a nonmatch returns empty even with cap zero. Working
/// and output allocation failures return OutputLimit without partial output.
///
/// PCRE failures, other locales, implicit allowlist policies, non-byte coercions, URL authorization,
/// output safety and public HTTP/DB integration are unmodeled. No product protocol
/// policy, site charset or shared RAM/RSS allowance is selected here.
pub fn wp_kses_bad_protocol_once2_c_bytes(
    scheme: &[u8],
    allowed_protocols: &PhpArray,
    max_output_bytes: usize,
) -> Result<Vec<u8>, BadProtocolError> {
    if allowed_protocols
        .iter()
        .any(|(_, value)| !matches!(value, PhpValue::Bytes(_)))
    {
        return Err(BadProtocolError::UnsupportedProtocolValue);
    }
    let mut decoded =
        wp_kses_decode_entities_byte_values(scheme, scheme.len()).map_err(|error| match error {
            EntityDecodeError::UnsupportedEntityValue => BadProtocolError::UnsupportedEntityValue,
            EntityDecodeError::InputLimit => BadProtocolError::InputLimit,
            EntityDecodeError::OutputLimit => BadProtocolError::OutputLimit,
        })?;
    decoded.retain(|byte| !matches!(byte, b'\t'..=b'\r' | b' '));
    let mut normalized = wp_kses_no_null_bytes(&decoded, SlashZeroMode::Remove, decoded.len())
        .map_err(|_| BadProtocolError::OutputLimit)?;
    drop(decoded);
    normalized.make_ascii_lowercase();
    let allowed = allowed_protocols.iter().any(|(_, value)| {
        matches!(value, PhpValue::Bytes(protocol) if protocol.eq_ignore_ascii_case(&normalized))
    });
    if !allowed {
        return Ok(Vec::new());
    }
    let output_length = normalized
        .len()
        .checked_add(1)
        .filter(|length| *length <= max_output_bytes)
        .ok_or(BadProtocolError::OutputLimit)?;
    normalized
        .try_reserve_exact(output_length - normalized.len())
        .map_err(|_| BadProtocolError::OutputLimit)?;
    normalized.push(b':');
    Ok(normalized)
}

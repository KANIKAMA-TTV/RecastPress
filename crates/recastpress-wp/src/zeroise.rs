//! Bounded integer/byte-string subset of pinned WordPress `zeroise`.

use crate::value::PhpValue;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZeroiseError {
    UnsupportedValue,
    /// Prototype host width bound, not PHP sprintf behavior.
    WidthLimit,
    /// Complete output byte cap or failed output allocation.
    OutputLimit,
}

impl fmt::Display for ZeroiseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedValue => "zeroise input is outside the supported value domain",
            Self::WidthLimit => "zeroise width exceeds the host bound",
            Self::OutputLimit => "zeroise output exceeds the host byte or allocation bound",
        })
    }
}

impl std::error::Error for ZeroiseError {}

/// WordPress 7.1.3 `formatting.php:2792` integer/Bytes subset of
/// `sprintf('%0' . $threshold . 's', $number)`, returning complete bytes.
///
/// Signed i64 values become decimal bytes on the stack; byte strings are neither
/// parsed nor normalized. Padding precedes the entire string, including a minus
/// sign, and a minimum width never truncates input. Lengths count bytes.
///
/// Width 0..=4096 and the caller's complete-output cap are prototype host bounds,
/// distinct from PHP formatting semantics and product capacity settings. Input
/// type and width are checked before the cap and output allocation; allocation
/// failure returns OutputLimit without partial output. Negative widths, floats,
/// bool/null/array/object coercion and diagnostics are not modeled. There is no
/// shared RAM reservation, escaping or output-safety guarantee.
pub fn zeroise(
    value: &PhpValue,
    width: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, ZeroiseError> {
    let mut decimal = [0u8; 20];
    let input = match value {
        PhpValue::Integer(number) => {
            let mut magnitude = number.unsigned_abs();
            let mut start = decimal.len();
            loop {
                start -= 1;
                decimal[start] = b'0' + (magnitude % 10) as u8;
                magnitude /= 10;
                if magnitude == 0 {
                    break;
                }
            }
            if *number < 0 {
                start -= 1;
                decimal[start] = b'-';
            }
            &decimal[start..]
        }
        PhpValue::Bytes(bytes) => bytes.as_slice(),
        _ => return Err(ZeroiseError::UnsupportedValue),
    };
    if width > 4096 {
        return Err(ZeroiseError::WidthLimit);
    }
    // max requires no potentially overflowing length addition.
    let output_len = input.len().max(width);
    if output_len > max_output_bytes {
        return Err(ZeroiseError::OutputLimit);
    }
    let padding = output_len
        .checked_sub(input.len())
        .ok_or(ZeroiseError::OutputLimit)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| ZeroiseError::OutputLimit)?;
    output.resize(padding, b'0');
    output.extend_from_slice(input);
    Ok(output)
}

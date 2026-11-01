//! Selected integer casts, with unsupported coercions kept explicit.

use crate::{CompatibilityError, value::PhpValue};

/// Supported absint coercions. Finite floats in [-2^63, 2^63) truncate toward
/// zero. Other floats, fraction/exponent/overflow strings and object/reference
/// behavior remain unsupported; PHP's float-cast diagnostics are not reproduced.
pub fn absint(value: &PhpValue) -> Result<PhpValue, CompatibilityError> {
    Ok(absint_integer(supported_integer(value)?))
}

pub(crate) fn supported_integer(value: &PhpValue) -> Result<i64, CompatibilityError> {
    match value {
        PhpValue::Null => Ok(0),
        PhpValue::Bool(value) => Ok(i64::from(*value)),
        PhpValue::Integer(value) => Ok(*value),
        PhpValue::Float(value)
            if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(value) =>
        {
            Ok(value.trunc() as i64)
        }
        PhpValue::Array(value) => Ok(i64::from(!value.is_empty())),
        PhpValue::Bytes(value) => {
            let whitespace = |byte: &u8| matches!(byte, b'\t'..=b'\r' | b' ');
            let start = value
                .iter()
                .position(|byte| !whitespace(byte))
                .unwrap_or(value.len());
            let end = value
                .iter()
                .rposition(|byte| !whitespace(byte))
                .map_or(start, |index| index + 1);
            let value = &value[start..end];
            if value.is_empty() {
                return Ok(0);
            }
            std::str::from_utf8(value)
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .ok_or(CompatibilityError::UnsupportedValue)
        }
        _ => Err(CompatibilityError::UnsupportedValue),
    }
}

/// absint's integer-input kernel. PHP's abs(PHP_INT_MIN) is a float containing
/// exactly 2^63, not a wrapped negative integer or a clamped PHP_INT_MAX.
pub fn absint_integer(value: i64) -> PhpValue {
    if value == i64::MIN {
        PhpValue::Float(9_223_372_036_854_775_808.0)
    } else {
        PhpValue::Integer(value.abs())
    }
}

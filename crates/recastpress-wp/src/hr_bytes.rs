//! Nonnegative decimal-byte subset of pinned WordPress `wp_convert_hr_to_bytes`.

use crate::CompatibilityError;

/// Convert ASCII `[0-9]+` with an optional single `k`, `m` or `g` suffix
/// (either case), using the fixed default 1024/1048576/1073741824 unit profile.
/// Leading zeros are accepted; at most 256 significant digits are supported
/// (all-zero input has zero significant digits). This compatibility-domain bound
/// excludes PHP's huge numeric strings whose intermediate infinity casts to zero.
/// It is not a PHP width limit, product capacity limit or memory allowance.
/// Every input byte is validated before counting significant digits or parsing;
/// empty input, signs, whitespace, decimal points, exponents and junk are
/// unsupported rather than invoking PHP's broader trim/cast/contains rules.
///
/// Pinned WordPress 7.1.3 `load.php:1687` returns
/// `min($bytes, PHP_INT_MAX)` in that argument order. On the fixed PHP 8.5.12
/// 64-bit oracle, even a positive product rounded to float 2^63 returns integer
/// PHP_INT_MAX in this order (the reversed min can return float). Within this
/// grammar, nonnegative parsing and multiplication therefore saturate to i64::MAX
/// without doing any float arithmetic. This is not a claim about other PHP
/// versions, negative overflow or arbitrary numeric-string conversions.
///
/// Stateless and allocation-free. No ini values, memory limits or product
/// capacity policy are read or selected; no value means "unlimited" here.
pub fn wp_convert_hr_to_bytes_default_units_bytes(input: &[u8]) -> Result<i64, CompatibilityError> {
    let (digits, multiplier) = match input.last() {
        Some(b'k' | b'K') => (&input[..input.len() - 1], 1024),
        Some(b'm' | b'M') => (&input[..input.len() - 1], 1024 * 1024),
        Some(b'g' | b'G') => (&input[..input.len() - 1], 1024 * 1024 * 1024),
        _ => (input, 1),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let significant_digits = digits
        .iter()
        .position(|byte| *byte != b'0')
        .map_or(0, |first| digits.len() - first);
    if significant_digits > 256 {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let value = digits.iter().fold(0i64, |value, byte| {
        value
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'))
    });
    Ok(value.saturating_mul(multiplier))
}

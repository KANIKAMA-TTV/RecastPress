//! Integer-input kernel of wp_checkdate and its explicit filter boundary.

/// Gregorian date validation for integer components, including PHP checkdate's
/// year range 1..=32767. source_date is forwarded unchanged to the filter; this
/// API does not parse it. Numeric string/float coercion is not implemented.
pub fn wp_checkdate_integers<T>(
    month: i64,
    day: i64,
    year: i64,
    source_date: &[u8],
    filter: impl FnOnce(bool, &[u8]) -> T,
) -> T {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    let valid = (1..=32767).contains(&year) && (1..=days).contains(&day);
    filter(valid, source_date)
}

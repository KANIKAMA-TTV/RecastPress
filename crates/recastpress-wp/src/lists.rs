//! Selected WordPress list primitives preserving PHP keys and value boundaries.

use crate::{
    CompatibilityError,
    value::{PhpArray, PhpKey, PhpValue},
};
use std::collections::BTreeSet;

/// String/array/bool/integer ID-list subset using wp_parse_list and supported absint
/// coercions. Preserves first keys and does not reindex after deduplication.
/// Root Null/float behavior and its diagnostics remain unsupported.
pub fn wp_parse_id_list(input: &PhpValue) -> Result<PhpArray, CompatibilityError> {
    let parsed = crate::wp_parse_list(input)?;
    let mut seen = BTreeSet::new();
    let mut result = PhpArray::new();
    for (key, value) in parsed.iter() {
        let integer = crate::numbers::supported_integer(value)?;
        if seen.insert(integer.unsigned_abs()) {
            result.insert(key.clone(), crate::absint_integer(integer));
        }
    }
    Ok(result)
}

/// Array/integer subset of wp_parse_id_list. As in wp_parse_list, null and
/// nested arrays are omitted. Other scalar coercions explicitly remain pending.
/// Retains the FIRST original key for each absolute integer magnitude, including
/// holes/string keys. PHP's array_unique uses string comparison; for this narrow
/// domain distinct magnitudes remain distinct even when float rounding would
/// otherwise conflate abs(PHP_INT_MIN) with PHP_INT_MAX.
pub fn wp_parse_id_list_integers(input: &PhpArray) -> Result<PhpArray, CompatibilityError> {
    let mut seen = BTreeSet::new();
    let mut result = PhpArray::new();
    for (key, value) in input.iter() {
        match value {
            PhpValue::Null | PhpValue::Array(_) => {}
            PhpValue::Integer(value) => {
                if seen.insert(value.unsigned_abs()) {
                    result.insert(key.clone(), crate::absint_integer(*value));
                }
            }
            _ => return Err(CompatibilityError::UnsupportedValue),
        }
    }
    Ok(result)
}

/// Extract a field from array entries. A falsey index selector (None, 0, "0",
/// or "") preserves the original list keys. Otherwise non-null index fields
/// become keys; absent/null indices append using PHP 8.3+ integer-key rules.
/// Duplicate indices overwrite values without moving their first position.
///
/// Objects/references, missing-field warnings, invalid-row diagnostics, float
/// index coercion and append overflow are not modeled; they return an error.
pub fn wp_list_pluck(
    input: &PhpValue,
    field: &PhpKey,
    index_key: Option<&PhpKey>,
) -> Result<PhpArray, CompatibilityError> {
    let mut output = PhpArray::new();
    let input = match input {
        PhpValue::Array(input) => input,
        PhpValue::Unsupported => return Err(CompatibilityError::UnsupportedValue),
        _ => return Ok(output),
    };
    let index_key = index_key.filter(|key| match key {
        PhpKey::Integer(key) => *key != 0,
        PhpKey::Bytes(key) => !key.is_empty() && key != b"0",
    });
    let mut maximum_integer: Option<i64> = None;
    for (original_key, row) in input.iter() {
        let PhpValue::Array(row) = row else {
            return Err(CompatibilityError::UnsupportedValue);
        };
        let value = match row.get(field) {
            None | Some(PhpValue::Unsupported) => return Err(CompatibilityError::UnsupportedValue),
            Some(value) => value.clone(),
        };
        let key = if let Some(index_key) = index_key {
            match row.get(index_key) {
                None | Some(PhpValue::Null) => PhpKey::Integer(match maximum_integer {
                    None => 0,
                    Some(maximum) => maximum
                        .checked_add(1)
                        .ok_or(CompatibilityError::UnsupportedValue)?,
                }),
                Some(PhpValue::Integer(value)) => PhpKey::Integer(*value),
                Some(PhpValue::Bool(value)) => PhpKey::Integer(i64::from(*value)),
                Some(PhpValue::Bytes(value)) => PhpKey::from_bytes(value),
                _ => return Err(CompatibilityError::UnsupportedValue),
            }
        } else {
            original_key.clone()
        };
        if let PhpKey::Integer(key) = &key {
            maximum_integer = Some(maximum_integer.map_or(*key, |maximum| maximum.max(*key)));
        }
        output.insert(key, value);
    }
    Ok(output)
}

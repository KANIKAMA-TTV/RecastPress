//! Native request-local adapters. Callback transport across Wasm is still pending.

use crate::{
    CompatibilityError,
    dispatcher::Dispatcher,
    hooks::HookError,
    value::{PhpArray, PhpValue},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilteredError {
    Unsupported(CompatibilityError),
    Callback(HookError),
}

/// wp_parse_str's supported byte-string grammar with its uncoerced filter return.
/// The native result replaces PHP's by-reference output parameter.
pub fn wp_parse_str(dispatcher: &mut Dispatcher, input: &[u8]) -> Result<PhpValue, FilteredError> {
    let parsed = crate::query::parse_query_bytes(input).map_err(FilteredError::Unsupported)?;
    dispatcher
        .apply_filters("wp_parse_str", PhpValue::Array(parsed), &[])
        .map_err(FilteredError::Callback)
}

/// Array or supported byte-query args with array defaults. Only the string
/// branch invokes wp_parse_str. Non-array filter results pass through with
/// empty defaults; a merge requiring an array is explicitly unsupported.
pub fn wp_parse_args(
    dispatcher: &mut Dispatcher,
    args: &PhpValue,
    defaults: &PhpArray,
) -> Result<PhpValue, FilteredError> {
    let parsed = match args {
        PhpValue::Array(array) => PhpValue::Array(
            crate::wp_parse_args_arrays(array, &PhpArray::new())
                .map_err(FilteredError::Unsupported)?,
        ),
        PhpValue::Bytes(bytes) => wp_parse_str(dispatcher, bytes)?,
        _ => {
            return Err(FilteredError::Unsupported(
                CompatibilityError::UnsupportedValue,
            ));
        }
    };
    if defaults.is_empty() {
        return Ok(parsed);
    }
    let PhpValue::Array(array) = parsed else {
        return Err(FilteredError::Unsupported(
            CompatibilityError::UnsupportedValue,
        ));
    };
    crate::wp_parse_args_arrays(&array, defaults)
        .map(PhpValue::Array)
        .map_err(FilteredError::Unsupported)
}

/// Byte-string sanitize_key with the named filter's uncoerced return value.
pub fn sanitize_key(dispatcher: &mut Dispatcher, key: &[u8]) -> Result<PhpValue, HookError> {
    crate::sanitize_key_bytes(key, |clean, raw| {
        dispatcher.apply_filters(
            "sanitize_key",
            PhpValue::Bytes(clean),
            &[PhpValue::Bytes(raw.to_vec())],
        )
    })
}

/// Integer-component wp_checkdate. source_date is passed through, never parsed.
pub fn wp_checkdate(
    dispatcher: &mut Dispatcher,
    month: i64,
    day: i64,
    year: i64,
    source_date: &[u8],
) -> Result<PhpValue, HookError> {
    crate::wp_checkdate_integers(month, day, year, source_date, |valid, raw| {
        dispatcher.apply_filters(
            "wp_checkdate",
            PhpValue::Bool(valid),
            &[PhpValue::Bytes(raw.to_vec())],
        )
    })
}

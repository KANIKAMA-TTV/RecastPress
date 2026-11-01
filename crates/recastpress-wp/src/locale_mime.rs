//! Byte-input adapters for WordPress 7.1.3 formatting.php:2468 and :5800.
//! These reproduce character whitelists and native request-local filters, not
//! MIME/RFC validity, OS locale validation, HTTP header setting or escaping.

use crate::{dispatcher::Dispatcher, hooks::HookError, value::PhpValue};

/// Keeps ASCII letters/digits, underscore and hyphen, preserving their case.
/// The named filter receives (clean, raw) and its typed result is not coerced.
/// Non-byte PHP coercions/diagnostics and Wasm callback transport are pending.
pub fn sanitize_locale_name(
    dispatcher: &mut Dispatcher,
    input: &[u8],
) -> Result<PhpValue, HookError> {
    let clean = input
        .iter()
        .copied()
        .filter(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        .collect();
    dispatcher.apply_filters(
        "sanitize_locale_name",
        PhpValue::Bytes(clean),
        &[PhpValue::Bytes(input.to_vec())],
    )
}

/// Keeps exactly the source regex's ASCII letters/digits and `-+*./` bytes.
/// Empty, wildcard-only and malformed MIME shapes can survive this whitelist.
/// The filter may return any supported PhpValue, including null or arrays.
/// This API does not set headers or implement non-byte PHP coercions/diagnostics.
pub fn sanitize_mime_type(
    dispatcher: &mut Dispatcher,
    input: &[u8],
) -> Result<PhpValue, HookError> {
    let clean = input
        .iter()
        .copied()
        .filter(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'*' | b'.' | b'/')
        })
        .collect();
    dispatcher.apply_filters(
        "sanitize_mime_type",
        PhpValue::Bytes(clean),
        &[PhpValue::Bytes(input.to_vec())],
    )
}

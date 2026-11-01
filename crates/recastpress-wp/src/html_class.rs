//! Byte inputs for WordPress 7.1.3 formatting.php:2438, sanitize_html_class.
//! Native request-local filters preserve typed results; no output escaping.

use crate::{dispatcher::Dispatcher, hooks::HookError, value::PhpValue};

/// Sanitizes byte classname/fallback inputs and calls the terminal named filter.
///
/// Percent escapes are removed, never decoded or rescanned, before retaining
/// ASCII letters/digits/underscore/hyphen. Only empty and `0` byte fallbacks are
/// PHP-false. A fallback call uses its own default empty fallback and replaces
/// the outer call entirely, including the filter's raw arguments. Non-byte PHP
/// coercions/diagnostics and Wasm callback transport remain outside this API.
pub fn sanitize_html_class(
    dispatcher: &mut Dispatcher,
    classname: &[u8],
    fallback: &[u8],
) -> Result<PhpValue, HookError> {
    let mut sanitized = clean_class_bytes(classname);
    let (raw, terminal_fallback) =
        if sanitized.is_empty() && !fallback.is_empty() && fallback != b"0" {
            sanitized = clean_class_bytes(fallback);
            (fallback, b"".as_slice())
        } else {
            (classname, fallback)
        };
    dispatcher.apply_filters(
        "sanitize_html_class",
        PhpValue::Bytes(sanitized),
        &[
            PhpValue::Bytes(raw.to_vec()),
            PhpValue::Bytes(terminal_fallback.to_vec()),
        ],
    )
}

fn clean_class_bytes(input: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while cursor < input.len() {
        let byte = input[cursor];
        if byte == b'%'
            && input
                .get(cursor + 1..cursor + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
        {
            cursor += 3;
        } else {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') {
                result.push(byte);
            }
            cursor += 1;
        }
    }
    result
}

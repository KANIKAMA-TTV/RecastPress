//! WordPress JSONP callback byte predicate for the C-locale comparison profile.

use crate::value::PhpValue;

/// Matches the fixed source's byte-regex predicate in the C locale. Empty names,
/// dots alone and leading digits are accepted; this is not a JavaScript parser,
/// an output-escaping interface, or authorization to expose a JSONP endpoint.
/// Non-C locale character classes and PHP object/reference modeling are pending.
pub fn wp_check_jsonp_callback(callback: &PhpValue) -> bool {
    match callback {
        PhpValue::Bytes(bytes) => bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.')),
        _ => false,
    }
}

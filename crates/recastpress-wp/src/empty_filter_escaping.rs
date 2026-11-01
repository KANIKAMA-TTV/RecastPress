//! Byte-return subset of esc_html/esc_attr for explicit UTF-8 and empty filters.
//!
//! No site option, mutable hook registry, Dispatcher or filter bookkeeping is
//! modeled. The reference PHP still calls apply_filters, even for empty output;
//! this module models only the returned bytes when its registry is empty.

use crate::{
    SpecialCharsError, SpecialCharsQuoteStyle, wp_is_valid_utf8_bytes, wp_specialchars_utf8_bytes,
};

fn escape(input: &[u8], max_output_bytes: usize) -> Result<Vec<u8>, SpecialCharsError> {
    // Equivalent to the existing UTF-8 wp_check_invalid_utf8 Bytes subset with
    // strip=false, but borrow valid bytes instead of allocating its copy. This
    // whole-input check must precede _wp_specialchars' five-special-byte guard.
    let checked = if wp_is_valid_utf8_bytes(input) {
        input
    } else {
        &[]
    };
    wp_specialchars_utf8_bytes(
        checked,
        SpecialCharsQuoteStyle::Quotes,
        false,
        max_output_bytes,
    )
}

/// WordPress 7.1.3 formatting.php:4751, explicit UTF-8, no-mbstring validation,
/// strip=false and an empty filter registry. Invalid UTF-8 returns empty even
/// when no ASCII special character occurs. Then use the fixed HTML normalizer
/// and HTML401 Quotes stage without double encoding. The real PHP hook is
/// `esc_html`; neither its count nor any callback execution is modeled here.
///
/// The final cap is a host policy. The helper's normalization intermediate is
/// unpooled and precedes that cap; output allocation is fallible with no partial
/// return. Capacities, RSS/peak/shared pools, site charset, PHP casts, other
/// encodings, mutable allowlists, filters, output-context safety and public
/// HTTP/DB integration remain outside this profile.
pub fn esc_html_utf8_empty_filters_bytes(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, SpecialCharsError> {
    escape(input, max_output_bytes)
}

/// WordPress 7.1.3 formatting.php:4776, with the same byte-return and resource
/// scope as esc_html_utf8_empty_filters_bytes. The real PHP filter is named
/// `attribute_escape`, not `esc_attr`. Empty-registry output is identical to
/// esc_html, but this does not connect a Dispatcher or model filter side effects.
pub fn esc_attr_utf8_empty_filters_bytes(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, SpecialCharsError> {
    escape(input, max_output_bytes)
}

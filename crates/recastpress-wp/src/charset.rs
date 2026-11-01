//! Explicit byte-string subset of WordPress `is_utf8_charset`.

/// Recognizes only `UTF-8` and `UTF8`, ignoring ASCII letter case.
///
/// Derived from pinned WordPress 7.1.3 `functions.php:7715` and
/// `compat.php:102`. No trimming, Unicode case folding or NUL termination is
/// applied. This stateless predicate performs no allocation and does not read
/// site settings. The PHP null/omitted argument path and non-string inputs are
/// outside this byte-input subset; it does not select a product charset policy.
pub fn is_utf8_charset_bytes(charset: &[u8]) -> bool {
    charset.eq_ignore_ascii_case(b"UTF-8") || charset.eq_ignore_ascii_case(b"UTF8")
}

//! Integer-input subset of WordPress `valid_unicode`.

/// Pinned WordPress 7.1.3 `kses.php:2346` character predicate for signed i64.
///
/// This preserves the source's three inclusive ranges and HT/LF/CR exceptions.
/// FDD0..FDEF and supplementary-plane ending noncharacters remain accepted;
/// BMP FFFE/FFFF and surrogate codepoints are rejected. It does not reject all
/// Unicode noncharacters, parse UTF-8, normalize text, sanitize HTML or establish
/// output safety. Noninteger PHP casts and diagnostics are outside this subset.
/// Stateless and allocation-free; no KSES globals or product HTTP/DB integration.
pub fn valid_unicode_i64(value: i64) -> bool {
    matches!(value, 9 | 10 | 13 | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
}

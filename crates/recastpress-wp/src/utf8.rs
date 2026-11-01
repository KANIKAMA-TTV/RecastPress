//! Byte-string UTF-8 primitives. Callers explicitly choose whether to scrub;
//! raw database reads and other byte-preserving APIs never invoke these.

/// Well-formed shortest UTF-8, excluding surrogates and values above U+10FFFF.
/// Noncharacters and a leading BOM are valid UTF-8 encodings.
pub fn wp_is_valid_utf8_bytes(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

/// Replace maximal ill-formed subparts with U+FFFD, retaining valid bytes.
/// Only byte-string input is modeled, not PHP scalar coercion or diagnostics.
pub fn wp_scrub_utf8_bytes(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes).into_owned().into_bytes()
}

/// Byte-string `wp_check_invalid_utf8` for an explicitly fixed UTF-8 site
/// charset and the no-mbstring fallback profile.
///
/// Pinned WordPress 7.1.3 `formatting.php:1127` retains empty and valid input,
/// rejects invalid input when `strip` is false, and replaces maximal ill-formed
/// subparts with U+FFFD when true. No site settings are read or selected here.
/// Null/scalar coercion, a non-boolean strip argument, non-UTF-8 sites, mbstring
/// implementations and changing the site charset during execution are outside
/// this explicit profile. Raw database readers never invoke it automatically.
pub fn wp_check_invalid_utf8_utf8_no_mbstring_bytes(bytes: &[u8], strip: bool) -> Vec<u8> {
    if bytes.is_empty() || wp_is_valid_utf8_bytes(bytes) {
        bytes.to_vec()
    } else if strip {
        wp_scrub_utf8_bytes(bytes)
    } else {
        Vec::new()
    }
}

/// Match the fixed source's byte patterns even inside otherwise invalid UTF-8.
/// This does not classify an entire string as valid or safe for HTML/XML output.
pub fn wp_has_noncharacters_bytes(bytes: &[u8]) -> bool {
    bytes
        .windows(3)
        .any(|part| matches!(part, [0xef, 0xb7, 0x90..=0xaf] | [0xef, 0xbf, 0xbe..=0xbf]))
        || bytes.windows(4).any(|part| {
            matches!(
                part,
                [0xf0, 0x9f | 0xaf | 0xbf, 0xbf, 0xbe..=0xbf]
                    | [0xf1..=0xf3, 0x8f | 0x9f | 0xaf | 0xbf, 0xbf, 0xbe..=0xbf]
                    | [0xf4, 0x8f, 0xbf, 0xbe..=0xbf]
            )
        })
}

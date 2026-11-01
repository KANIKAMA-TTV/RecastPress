//! C-locale byte subset of pinned WordPress JSON media-type detection.

fn whitespace(byte: u8) -> bool {
    matches!(byte, b'\t'..=b'\r' | b' ')
}

fn prefix_byte(byte: u8) -> bool {
    // The source's &-\^ is a range, including slash, comma, semicolon,
    // brackets and backslash. It is not a conventional MIME token class.
    matches!(byte, b'!' | b'#' | b'$' | b'&'..=b'_' | b'a'..=b'z')
}

fn right_boundary(bytes: &[u8]) -> bool {
    bytes
        .first()
        .is_none_or(|byte| whitespace(*byte) || matches!(byte, b';' | b','))
}

fn json_tail(bytes: &[u8]) -> bool {
    if !bytes
        .get(..4)
        .is_some_and(|part| part.eq_ignore_ascii_case(b"json"))
    {
        return false;
    }
    let rest = &bytes[4..];
    right_boundary(rest)
        || (rest
            .get(..7)
            .is_some_and(|part| part.eq_ignore_ascii_case(b"+oembed"))
            && right_boundary(&rest[7..]))
}

/// Return the fixed WordPress 7.1.3 `load.php:1964` regex result for bytes
/// under the C locale. The search is unanchored and preserves the source's
/// unusual ASCII character range, whitespace boundaries and optional `+oembed`.
///
/// This stateless, allocation-free predicate does not model WordPress's internal
/// memoization, non-string diagnostics or PCRE resource failures. Other locales
/// are outside this subset. It does not parse MIME parameters, JSON content or
/// HTTP headers, authorize a request, sanitize UTF-8 or establish output safety.
pub fn wp_is_json_media_type_c_bytes(bytes: &[u8]) -> bool {
    // Keep the earliest content start in the current uninterrupted prefix-byte
    // run. Any later application/ start in that run is dominated for the
    // nonempty-prefix alternative; direct json is checked at every new start.
    // Each byte receives constant-sized lookahead, without rescanning a run.
    let mut prefix_start = None;
    for (index, &byte) in bytes.iter().enumerate() {
        if (index == 0 || whitespace(bytes[index - 1]) || bytes[index - 1] == b',')
            && bytes[index..]
                .get(..12)
                .is_some_and(|part| part.eq_ignore_ascii_case(b"application/"))
        {
            let start = index + 12;
            if json_tail(&bytes[start..]) {
                return true;
            }
            prefix_start.get_or_insert(start);
        }
        if byte == b'+'
            && prefix_start.is_some_and(|start| index > start)
            && json_tail(&bytes[index + 1..])
        {
            return true;
        }
        if !prefix_byte(byte) {
            prefix_start = None;
        }
    }
    false
}

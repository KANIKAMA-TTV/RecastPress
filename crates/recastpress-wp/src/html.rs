//! Byte-string subset of WordPress 7.1.3's HTML delimiter splitting.
//!
//! Source: wp-includes/formatting.php:615 (`wp_html_split`) and :626
//! (`get_html_split_regex`). This follows that regex, not HTML parsing rules.

/// Splits bytes into alternating text and captured HTML-like delimiters.
///
/// Leading/trailing and intervening empty text entries are preserved, matching
/// `PREG_SPLIT_DELIM_CAPTURE`. Only exact `<!--` and `<![CDATA[` prefixes select
/// special terminators. Other `<` sequences end at the first `>` even inside a
/// quoted attribute; a missing terminator consumes the remainder of the input.
/// PHP argument coercions and PCRE resource-limit failures are not modeled.
pub fn wp_html_split_bytes(input: &[u8]) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].iter().position(|&byte| byte == b'<') {
        let start = cursor + relative;
        result.push(input[cursor..start].to_vec());
        let after_open = &input[start + 1..];
        let end = if after_open.starts_with(b"!--") {
            // The regex consumes `!`, then dashes unless they begin `-->`.
            // Searching from the first dash also preserves the short `<!-->`.
            terminated_end(input, start + 2, b"-->")
        } else if after_open.starts_with(b"![CDATA[") {
            terminated_end(input, start + b"<![CDATA[".len(), b"]]>")
        } else {
            terminated_end(input, start + 1, b">")
        };
        result.push(input[start..end].to_vec());
        cursor = end;
    }
    result.push(input[cursor..].to_vec());
    result
}

fn terminated_end(input: &[u8], start: usize, terminator: &[u8]) -> usize {
    input[start..]
        .windows(terminator.len())
        .position(|window| window == terminator)
        .map_or(input.len(), |offset| start + offset + terminator.len())
}

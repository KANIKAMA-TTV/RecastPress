//! Byte-string subset of WordPress 7.1.3 formatting.php:4905.

/// Reproduces `wp_make_link_relative` for byte strings.
///
/// The source regex is `|^(https?:)?//[^/]+(/?.*)|i`. This strips its
/// nonempty authority-like run, including query/fragment/control bytes, rather
/// than parsing or validating a URL. PHP coercions and PCRE failures are outside
/// this kernel; it provides no escaping or authorization checks.
pub fn wp_make_link_relative_bytes(input: &[u8]) -> Vec<u8> {
    let authority_start = if input.starts_with(b"//") {
        2
    } else if input
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"http://"))
    {
        7
    } else if input
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"https://"))
    {
        8
    } else {
        return input.to_vec();
    };
    let authority = &input[authority_start..];
    if authority.is_empty() || authority[0] == b'/' {
        return input.to_vec();
    }
    let tail_start = authority
        .iter()
        .position(|&byte| byte == b'/')
        .map_or(input.len(), |offset| authority_start + offset);
    // `.*` can stop at LF, but preg_replace retains the unmatched suffix.
    // Group 2 plus that suffix is exactly the original tail from this slash.
    input[tail_start..].to_vec()
}

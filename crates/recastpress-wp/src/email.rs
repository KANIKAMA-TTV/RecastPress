//! Byte-input/default-deprecated=false subset of WordPress 7.1.3
//! wp-includes/formatting.php:3613 (`is_email`). This preserves WP's validation
//! order and quirks; it is not RFC-complete, international-domain support, DNS
//! verification, an escaping function or a security/deliverability guarantee.

use crate::{dispatcher::Dispatcher, hooks::HookError, value::PhpValue};

/// Applies the terminal native filter with (false/original email, raw, context).
/// Eight rejection contexts are byte strings; success context is PHP Null.
/// Filter results keep their modeled types. Non-byte coercions/diagnostics,
/// non-default deprecated arguments and Wasm callback transport are pending.
pub fn is_email(dispatcher: &mut Dispatcher, email: &[u8]) -> Result<PhpValue, HookError> {
    let context = rejection_context(email);
    let value = if context.is_some() {
        PhpValue::Bool(false)
    } else {
        PhpValue::Bytes(email.to_vec())
    };
    dispatcher.apply_filters(
        "is_email",
        value,
        &[
            PhpValue::Bytes(email.to_vec()),
            context.map_or(PhpValue::Null, |context| {
                PhpValue::Bytes(context.as_bytes().to_vec())
            }),
        ],
    )
}

fn php_trim_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\0' | 0x0b)
}

fn edge_trim_would_change(input: &[u8], extra: u8) -> bool {
    input
        .first()
        .into_iter()
        .chain(input.last())
        .any(|&byte| php_trim_byte(byte) || byte == extra)
}

fn local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'/'
                | b'='
                | b'?'
                | b'^'
                | b'_'
                | b'`'
                | b'{'
                | b'|'
                | b'}'
                | b'~'
                | b'.'
                | b'-'
        )
}

fn rejection_context(email: &[u8]) -> Option<&'static str> {
    if email.len() < 6 {
        return Some("email_too_short");
    }
    // This existence check intentionally differs from the first-@ split below.
    if !email[1..].contains(&b'@') {
        return Some("email_no_at");
    }
    let split = email
        .iter()
        .position(|&byte| byte == b'@')
        .expect("an @ was found after byte zero");
    let local = &email[..split];
    let domain = &email[split + 1..];
    // PCRE `$` also matches immediately before one final LF; the original local
    // bytes are retained, and that single LF is not a general trim operation.
    let local_match = local.strip_suffix(b"\n").unwrap_or(local);
    if local_match.is_empty() || !local_match.iter().all(|&byte| local_byte(byte)) {
        return Some("local_invalid_chars");
    }
    if domain.windows(2).any(|pair| pair == b"..") {
        return Some("domain_period_sequence");
    }
    if edge_trim_would_change(domain, b'.') {
        return Some("domain_period_limits");
    }
    if !domain.contains(&b'.') {
        return Some("domain_no_periods");
    }
    for sub in domain.split(|&byte| byte == b'.') {
        if edge_trim_would_change(sub, b'-') {
            return Some("sub_hyphen_limits");
        }
        // Any final LF would already have failed the preceding trim test.
        if sub.is_empty()
            || !sub
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        {
            return Some("sub_invalid_chars");
        }
    }
    None
}

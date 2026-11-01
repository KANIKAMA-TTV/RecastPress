//! Explicit-cookie wp_parse_auth_cookie byte subset from pinned pluggable.php.
//! Empty/`0` cookie requests use $_COOKIE, SSL and cookie-name constants in PHP
//! and are deliberately unsupported here. With an explicit cookie, scheme is
//! only an unchanged caller label. No hash/token/expiry validation, time policy,
//! superglobal lookup, scheme resolution or authentication is performed.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthCookieParseError {
    UnsupportedValue,
    InputLimit,
}

impl fmt::Display for AuthCookieParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "unsupported explicit auth cookie input",
            Self::InputLimit => "auth cookie parse input limit exceeded",
        })
    }
}

impl std::error::Error for AuthCookieParseError {}

/// Unvalidated borrowed fields, not an authenticated user/session/credential.
/// No Clone/Display/mutable or owned escape; Debug never prints raw fields.
/// Borrowed source buffers and explicit caller copies remain caller-owned.
pub struct ParsedAuthCookie<'a> {
    username: &'a [u8],
    expiration: &'a [u8],
    token: &'a [u8],
    hmac: &'a [u8],
    scheme: &'a [u8],
}

impl ParsedAuthCookie<'_> {
    pub fn username(&self) -> &[u8] {
        self.username
    }
    pub fn expiration_bytes(&self) -> &[u8] {
        self.expiration
    }
    pub fn token(&self) -> &[u8] {
        self.token
    }
    pub fn hmac_bytes(&self) -> &[u8] {
        self.hmac
    }
    pub fn scheme_label(&self) -> &[u8] {
        self.scheme
    }
}

impl fmt::Debug for ParsedAuthCookie<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ParsedAuthCookie([redacted])")
    }
}

/// Match the explicit-cookie branch's exact four-field explode, without heap
/// allocation. `None` means source false for a field count other than four.
/// Exactly three pipes yield Some even when every field is empty or invalid.
/// Expiration, token and HMAC remain raw strings, without typed conversion or
/// format checks. Scheme may be any bytes, including empty/unknown labels.
///
/// Empty/`0` cookie is refused BEFORE the mandatory input cap. Otherwise the cap
/// checks cookie.len()+scheme.len() before splitting. It bounds raw input bytes,
/// not borrowed storage, RSS, CPU, process lifetime or concurrent work. No output
/// cap is needed for this fixed-size view of five borrowed byte slices. This is
/// not HTTP-cookie/header admission, credential validation or principal proof.
pub fn wp_parse_auth_cookie_explicit_bytes<'a>(
    cookie: &'a [u8],
    scheme: &'a [u8],
    max_input_bytes: usize,
) -> Result<Option<ParsedAuthCookie<'a>>, AuthCookieParseError> {
    if cookie.is_empty() || cookie == b"0" {
        return Err(AuthCookieParseError::UnsupportedValue);
    }
    let input_bytes = cookie
        .len()
        .checked_add(scheme.len())
        .ok_or(AuthCookieParseError::InputLimit)?;
    if input_bytes > max_input_bytes {
        return Err(AuthCookieParseError::InputLimit);
    }
    let mut fields = cookie.split(|byte| *byte == b'|');
    let (Some(username), Some(expiration), Some(token), Some(hmac)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Ok(None);
    };
    if fields.next().is_some() {
        return Ok(None);
    }
    Ok(Some(ParsedAuthCookie {
        username,
        expiration,
        token,
        hmac,
        scheme,
    }))
}

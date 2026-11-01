//! Prepared GET/non-Ajax cookie matching only; no principal/session issuance.
//! Explicit now, caller-bound complete user/session views and resolved salt.
//! Preflight refusals are not WordPress false; no HTTP, clock or DB access.
#![forbid(unsafe_code)]
use crate::auth_cookie::{auth_cookie_mac, pass_fragment};
use crate::auth_cookie_parse::wp_parse_auth_cookie_explicit_bytes;
use hmac::Mac;
use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieVerificationError {
    UnsupportedValue,
    InputLimit,
    SessionLimit,
    InvalidSession,
    ProviderFailure,
}
impl fmt::Display for CookieVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("prepared cookie proof refused")
    }
}
impl std::error::Error for CookieVerificationError {}

/// Caller-attested existing row. Positive ID is not an existence/role proof.
/// Canonical row login is counted by the cap but never replaces cookie username.
pub struct PreparedCookieVerificationUser<'a> {
    id: i64,
    login: &'a [u8],
    hash: &'a [u8],
}
impl<'a> PreparedCookieVerificationUser<'a> {
    pub fn new(id: i64, login: &'a [u8], hash: &'a [u8]) -> Result<Self, CookieVerificationError> {
        if id <= 0 {
            return Err(CookieVerificationError::UnsupportedValue);
        }
        Ok(Self { id, login, hash })
    }
}
impl fmt::Debug for PreparedCookieVerificationUser<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedCookieVerificationUser([redacted])")
    }
}
#[derive(Clone, Copy)]
/// The legacy integer is wrapped as a record, including zero/negative values.
pub enum CookieSessionExpiration {
    Legacy(i64),
    Record(i64),
}
impl CookieSessionExpiration {
    fn value(self) -> i64 {
        match self {
            Self::Legacy(v) | Self::Record(v) => v,
        }
    }
}
/// One typed entry of a complete caller-provided session projection.
pub struct PreparedCookieSession<'a> {
    pub verifier: &'a [u8],
    pub expiration: CookieSessionExpiration,
}
/// Bounded complete unique-verifier map, bound to the prepared user's ID.
/// Constructor refusals are outside-profile admission, before source branches;
/// malformed unselected entries are not silently ignored. No DB/cache is read.
pub struct PreparedCookieSessions<'a> {
    owner: i64,
    entries: &'a [PreparedCookieSession<'a>],
    key_bytes: usize,
}
impl<'a> PreparedCookieSessions<'a> {
    /// Complete caller-provided map admission is preflight, NOT a WP false branch.
    pub fn new(
        owner: i64,
        entries: &'a [PreparedCookieSession<'a>],
        max_entries: usize,
        max_key_bytes: usize,
    ) -> Result<Self, CookieVerificationError> {
        if owner <= 0 {
            return Err(CookieVerificationError::UnsupportedValue);
        }
        if entries.len() > max_entries {
            return Err(CookieVerificationError::SessionLimit);
        }
        let bytes = entries
            .iter()
            .try_fold(0usize, |n, e| n.checked_add(e.verifier.len()))
            .ok_or(CookieVerificationError::SessionLimit)?;
        if bytes > max_key_bytes {
            return Err(CookieVerificationError::SessionLimit);
        }
        for (i, entry) in entries.iter().enumerate() {
            if entry.verifier.len() != 64
                || !entry
                    .verifier
                    .iter()
                    .all(|b| matches!(b,b'0'..=b'9'|b'a'..=b'f'))
                || entries[..i]
                    .iter()
                    .any(|old| old.verifier == entry.verifier)
            {
                return Err(CookieVerificationError::InvalidSession);
            }
        }
        Ok(Self {
            owner,
            entries,
            key_bytes: bytes,
        })
    }
}
impl fmt::Debug for PreparedCookieSessions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedCookieSessions([redacted])")
    }
}
#[derive(Clone, Copy)]
/// Mandatory logical input cap; no default, RSS/CPU/concurrency or pool promise.
/// Counts cookie+scheme+salt+canonical row login+FULL hash+all verifier key bytes.
/// Preowned storage and fixed-size typed integers/enums are excluded.
pub struct CookieVerificationLimits {
    max_input_bytes: usize,
}
impl CookieVerificationLimits {
    pub fn new(max_input_bytes: usize) -> Self {
        Self { max_input_bytes }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieRejectionStage {
    Malformed,
    Expired,
    UnknownUser,
    BadMac,
    BadSession,
}
/// Same borrowed prepared record, not a durable session, role or HTTP capability.
/// A later logout/row change is not continuously revalidated by retaining this.
pub struct UnfinalizedCookieMatch<'a> {
    _user: &'a PreparedCookieVerificationUser<'a>,
}
impl UnfinalizedCookieMatch<'_> {
    /// Raw prepared-record identity witness only, never an authenticated principal.
    pub fn prepared_record_id(&self) -> i64 {
        self._user.id
    }
}
impl fmt::Debug for UnfinalizedCookieMatch<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnfinalizedCookieMatch([redacted])")
    }
}
#[derive(Debug)]
pub enum CookieVerificationOutcome<'a> {
    NoMatch(CookieRejectionStage),
    Match(UnfinalizedCookieMatch<'a>),
}

/// Canonical signed i64 only, no PHP-general numeric-string coercion.
fn canonical_i64(raw: &[u8]) -> Result<i64, CookieVerificationError> {
    let (negative, digits) = if raw.first() == Some(&b'-') {
        (true, &raw[1..])
    } else {
        (false, raw)
    };
    if digits.is_empty()
        || digits.len() > 19
        || (digits.len() > 1 && digits[0] == b'0')
        || (negative && digits == b"0")
    {
        return Err(CookieVerificationError::UnsupportedValue);
    }
    let magnitude = digits
        .iter()
        .try_fold(0u64, |n, b| {
            if !b.is_ascii_digit() {
                None
            } else {
                n.checked_mul(10)?.checked_add(u64::from(*b - b'0'))
            }
        })
        .ok_or(CookieVerificationError::UnsupportedValue)?;
    if negative {
        if magnitude == 1u64 << 63 {
            Ok(i64::MIN)
        } else {
            i64::try_from(magnitude)
                .map(|n| -n)
                .map_err(|_| CookieVerificationError::UnsupportedValue)
        }
    } else {
        i64::try_from(magnitude).map_err(|_| CookieVerificationError::UnsupportedValue)
    }
}
fn hex(input: &[u8], output: &mut [u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (b, pair) in input.iter().zip(output.chunks_exact_mut(2)) {
        pair[0] = HEX[usize::from(*b >> 4)];
        pair[1] = HEX[usize::from(*b & 15)];
    }
}
fn token_verifier(token: &[u8]) -> [u8; 64] {
    let mut digest = Sha256::digest(token);
    let mut out = [0; 64];
    hex(&digest, &mut out);
    digest[..].zeroize();
    out
}
struct MacInput<'a> {
    username: &'a [u8],
    hash: &'a [u8],
    expiry: &'a [u8],
    token: &'a [u8],
    salt: &'a [u8],
    tag: &'a [u8],
}
fn verify_mac(v: MacInput<'_>) -> Result<bool, CookieVerificationError> {
    let outer = auth_cookie_mac(v.username, pass_fragment(v.hash), v.expiry, v.token, v.salt)
        .map_err(|_| CookieVerificationError::ProviderFailure)?;
    // PHP hash_equals compares LOWERCASE ASCII64, so uppercase is not normalized.
    if v.tag.len() != 64 || !v.tag.iter().all(|b| matches!(b,b'0'..=b'9'|b'a'..=b'f')) {
        return Ok(false);
    }
    let mut raw = Zeroizing::new([0u8; 32]);
    for (pair, out) in v.tag.chunks_exact(2).zip(raw.iter_mut()) {
        let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        *out = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    Ok(outer.verify_slice(&raw[..]).is_ok())
}

/// Explicit-now GET/non-Ajax subset; all profile/cap admissions are refusals.
/// No default clock/grace, user lookup/normalization, session writes or permission.
/// Expiry must be canonical signed-i64 bytes. Token empty/ASCII-zero is allowed,
/// unlike generation. A malformed/uppercase HMAC is NoMatch, not normalized.
/// Scratch wiping is best effort; provider/compiler/caller storage is not erased
/// completely and this function as a whole is not promised constant-time.
pub fn wp_validate_auth_cookie_prepared_get_non_ajax_resolved_salt_bytes<'a>(
    cookie: &[u8],
    scheme: &[u8],
    user: Option<&'a PreparedCookieVerificationUser<'a>>,
    sessions: &PreparedCookieSessions<'_>,
    salt: &[u8],
    now: i64,
    limits: CookieVerificationLimits,
) -> Result<CookieVerificationOutcome<'a>, CookieVerificationError> {
    check_with(
        cookie, scheme, user, sessions, salt, now, limits, verify_mac,
    )
}
#[allow(clippy::too_many_arguments)]
fn check_with<'a>(
    cookie: &[u8],
    scheme: &[u8],
    user: Option<&'a PreparedCookieVerificationUser<'a>>,
    sessions: &PreparedCookieSessions<'_>,
    salt: &[u8],
    now: i64,
    limits: CookieVerificationLimits,
    mac: impl FnOnce(MacInput<'_>) -> Result<bool, CookieVerificationError>,
) -> Result<CookieVerificationOutcome<'a>, CookieVerificationError> {
    if cookie.is_empty()
        || cookie == b"0"
        || salt.is_empty()
        || user.is_some_and(|u| u.id != sessions.owner)
    {
        return Err(CookieVerificationError::UnsupportedValue);
    }
    let mut total = sessions.key_bytes;
    for raw in [
        cookie,
        scheme,
        salt,
        user.map_or(&[][..], |u| u.login),
        user.map_or(&[][..], |u| u.hash),
    ] {
        total = total
            .checked_add(raw.len())
            .ok_or(CookieVerificationError::InputLimit)?;
    }
    if total > limits.max_input_bytes {
        return Err(CookieVerificationError::InputLimit);
    }
    let Some(parsed) = wp_parse_auth_cookie_explicit_bytes(cookie, scheme, limits.max_input_bytes)
        .map_err(|_| CookieVerificationError::UnsupportedValue)?
    else {
        return Ok(CookieVerificationOutcome::NoMatch(
            CookieRejectionStage::Malformed,
        ));
    };
    let expiry = canonical_i64(parsed.expiration_bytes())?;
    if expiry < now {
        return Ok(CookieVerificationOutcome::NoMatch(
            CookieRejectionStage::Expired,
        ));
    }
    let Some(user) = user else {
        return Ok(CookieVerificationOutcome::NoMatch(
            CookieRejectionStage::UnknownUser,
        ));
    };
    if !mac(MacInput {
        username: parsed.username(),
        hash: user.hash,
        expiry: parsed.expiration_bytes(),
        token: parsed.token(),
        salt,
        tag: parsed.hmac_bytes(),
    })? {
        return Ok(CookieVerificationOutcome::NoMatch(
            CookieRejectionStage::BadMac,
        ));
    }
    let verifier = Zeroizing::new(token_verifier(parsed.token()));
    if !sessions
        .entries
        .iter()
        .any(|s| s.verifier == &verifier[..] && s.expiration.value() >= now)
    {
        return Ok(CookieVerificationOutcome::NoMatch(
            CookieRejectionStage::BadSession,
        ));
    }
    Ok(CookieVerificationOutcome::Match(UnfinalizedCookieMatch {
        _user: user,
    }))
}

#[cfg(test)]
#[path = "auth_cookie_verify_contract.rs"]
mod tests;

//! wp_generate_auth_cookie return-byte subset from pinned pluggable.php:951.
//! The caller supplies an existing user, an already resolved salt and a nonempty
//! PHP-truthy token, with auth_cookie and `all` filters absent. No user lookup,
//! salt/scheme resolution, session creation, RNG, time policy, cookie parsing,
//! HTTP header validation, authentication or principal issuance is performed.

use hmac::{Hmac as ShaHmac, Mac as ShaMac};
use hmac_md5::{Hmac as Md5Hmac, KeyInit as Md5KeyInit, Mac as Md5Mac};
use md5::Md5;
use sha2::Sha256;
use std::fmt;
use zeroize::{Zeroize, Zeroizing};

/// Caller-prepared existing-user fields. Positive ID is only a subset guard,
/// not an existence/identity/authorization proof, and is not retained. Login and
/// password hash remain raw bytes; no UTF-8, trim, delimiter or hash-format
/// validation is applied. Their association is the caller's responsibility.
pub struct PreparedAuthCookieUser<'a> {
    login: &'a [u8],
    password_hash: &'a [u8],
}

impl<'a> PreparedAuthCookieUser<'a> {
    pub fn new(id: i64, login: &'a [u8], password_hash: &'a [u8]) -> Result<Self, AuthCookieError> {
        if id <= 0 {
            return Err(AuthCookieError::UnsupportedValue);
        }
        Ok(Self {
            login,
            password_hash,
        })
    }
}

impl fmt::Debug for PreparedAuthCookieUser<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedAuthCookieUser([redacted])")
    }
}

/// Caller-selected logical-byte caps, with no default or product configuration.
/// Input counts login + FULL password hash + resolved salt + token, including
/// bytes not used in the selected hash fragment. Typed ID/expiration and fixed
/// framing are excluded. Output counts the complete returned cookie bytes.
/// Borrowed input storage, provider state, Vec capacity, peak/RSS/CPU/shared
/// budgets and concurrency are not measured by these limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthCookieLimits {
    max_input_bytes: usize,
    max_output_bytes: usize,
}

impl AuthCookieLimits {
    pub fn new(max_input_bytes: usize, max_output_bytes: usize) -> Self {
        Self {
            max_input_bytes,
            max_output_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthCookieError {
    UnsupportedValue,
    InputLimit,
    OutputLimit,
    AllocationFailed,
    ProviderFailure,
}

impl fmt::Display for AuthCookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "unsupported prepared auth cookie input",
            Self::InputLimit => "auth cookie input limit exceeded",
            Self::OutputLimit => "auth cookie output limit exceeded",
            Self::AllocationFailed => "auth cookie allocation failed",
            Self::ProviderFailure => "auth cookie provider failed",
        })
    }
}

impl std::error::Error for AuthCookieError {}

/// Move-only raw cookie owner, not an HTTP HeaderValue or a validated session.
/// No Display, Clone, Deref or owned/mutable escape. Borrowing and explicit
/// caller copies are outside this owner's best-effort wipe on drop.
pub struct WpAuthCookie {
    bytes: Zeroizing<Vec<u8>>,
}

impl WpAuthCookie {
    pub fn as_cookie_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl fmt::Debug for WpAuthCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WpAuthCookie([redacted])")
    }
}

pub(crate) fn pass_fragment(hash: &[u8]) -> &[u8] {
    if hash.starts_with(b"$P$") || hash.starts_with(b"$2y$") {
        if hash.len() <= 8 {
            b""
        } else {
            &hash[8..hash.len().min(12)]
        }
    } else {
        &hash[hash.len().saturating_sub(4)..]
    }
}

fn decimal_expiration(expiration: i64, scratch: &mut [u8; 20]) -> &[u8] {
    let mut magnitude = expiration.unsigned_abs();
    let mut at = scratch.len();
    loop {
        at -= 1;
        scratch[at] = b'0' + (magnitude % 10) as u8;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if expiration < 0 {
        at -= 1;
        scratch[at] = b'-';
    }
    &scratch[at..]
}

fn lower_hex(input: &[u8], output: &mut [u8]) -> Result<(), AuthCookieError> {
    if input.len().checked_mul(2) != Some(output.len()) {
        return Err(AuthCookieError::ProviderFailure);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (byte, pair) in input.iter().zip(output.chunks_exact_mut(2)) {
        pair[0] = HEX[usize::from(*byte >> 4)];
        pair[1] = HEX[usize::from(*byte & 15)];
    }
    Ok(())
}

// Shared construction only: generation owns its falsey-token guard; prepared
// verification must allow empty/ASCII-zero tokens and use raw cookie fields.
pub(crate) fn auth_cookie_mac(
    login: &[u8],
    fragment: &[u8],
    expiration: &[u8],
    token: &[u8],
    salt: &[u8],
) -> Result<ShaHmac<Sha256>, AuthCookieError> {
    let mut inner = <Md5Hmac<Md5> as Md5KeyInit>::new_from_slice(salt)
        .map_err(|_| AuthCookieError::ProviderFailure)?;
    for part in [login, b"|", fragment, b"|", expiration, b"|", token] {
        inner.update(part);
    }
    let mut digest = inner.finalize().into_bytes();
    let mut key = Zeroizing::new([0_u8; 32]);
    let encoded = lower_hex(digest.as_slice(), key.as_mut_slice());
    digest.as_mut_slice().zeroize();
    encoded?;

    // wp_hash returns LOWERCASE HEX ASCII32, not the raw 16-byte MD5 digest.
    let mut outer = <ShaHmac<Sha256> as ShaMac>::new_from_slice(key.as_slice())
        .map_err(|_| AuthCookieError::ProviderFailure)?;
    for part in [login, b"|", expiration, b"|", token] {
        outer.update(part);
    }
    Ok(outer)
}

fn cookie_hmac(
    login: &[u8],
    fragment: &[u8],
    expiration: &[u8],
    token: &[u8],
    salt: &[u8],
) -> Result<Zeroizing<[u8; 64]>, AuthCookieError> {
    let outer = auth_cookie_mac(login, fragment, expiration, token, salt)?;
    let mut digest = outer.finalize().into_bytes();
    let mut hex = Zeroizing::new([0_u8; 64]);
    let encoded = lower_hex(&digest[..], hex.as_mut_slice());
    digest[..].zeroize();
    encoded?;
    Ok(hex)
}

fn generate_with(
    user: &PreparedAuthCookieUser<'_>,
    expiration: i64,
    token: &[u8],
    salt: &[u8],
    limits: AuthCookieLimits,
    provider: impl FnOnce(
        &[u8],
        &[u8],
        &[u8],
        &[u8],
        &[u8],
    ) -> Result<Zeroizing<[u8; 64]>, AuthCookieError>,
) -> Result<WpAuthCookie, AuthCookieError> {
    if token.is_empty() || token == b"0" || salt.is_empty() {
        return Err(AuthCookieError::UnsupportedValue);
    }
    let input_bytes = user
        .login
        .len()
        .checked_add(user.password_hash.len())
        .and_then(|length| length.checked_add(token.len()))
        .and_then(|length| length.checked_add(salt.len()))
        .ok_or(AuthCookieError::InputLimit)?;
    if input_bytes > limits.max_input_bytes {
        return Err(AuthCookieError::InputLimit);
    }
    let mut scratch = Zeroizing::new([0_u8; 20]);
    let decimal = decimal_expiration(expiration, &mut scratch);
    let output_bytes = user
        .login
        .len()
        .checked_add(decimal.len())
        .and_then(|length| length.checked_add(token.len()))
        .and_then(|length| length.checked_add(67))
        .ok_or(AuthCookieError::OutputLimit)?;
    if output_bytes > limits.max_output_bytes {
        return Err(AuthCookieError::OutputLimit);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(output_bytes)
        .map_err(|_| AuthCookieError::AllocationFailed)?;
    let digest = provider(
        user.login,
        pass_fragment(user.password_hash),
        decimal,
        token,
        salt,
    )?;
    for part in [
        user.login,
        b"|",
        decimal,
        b"|",
        token,
        b"|",
        digest.as_slice(),
    ] {
        bytes.extend_from_slice(part);
    }
    Ok(WpAuthCookie { bytes })
}

/// Fixed default-algorithm return bytes with caller-resolved salt and no cookie
/// filters. Token ''/'0' are refused before caps/providers: the source would
/// create a session instead. Empty salt is outside this profile; salt '0' is a
/// nonempty raw salt. Expiration is signed decimal without any time validation.
/// Prefix/fragment slicing is byte-based even for malformed or short hashes.
/// Final framing preserves login/token pipes, NULs and other raw bytes; this is
/// deliberately not HTTP-header or parseable-cookie admission.
///
/// Streaming HMAC avoids concatenated secret message buffers. Owned decimal,
/// digest/hex/key and output buffers are wiped best-effort. HMAC-MD5's provider
/// zeroize feature is enabled. Outer HMAC-SHA256 (hmac 0.12) internal state,
/// caller inputs, compiler temporaries and allocator memory are NOT guaranteed
/// erased. No constant-time login/validation/session behavior is claimed.
pub fn wp_generate_auth_cookie_prepared_resolved_salt_empty_filters_bytes(
    user: &PreparedAuthCookieUser<'_>,
    expiration: i64,
    token: &[u8],
    resolved_salt: &[u8],
    limits: AuthCookieLimits,
) -> Result<WpAuthCookie, AuthCookieError> {
    generate_with(user, expiration, token, resolved_salt, limits, cookie_hmac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refused_profile_and_caps_never_invoke_provider_and_provider_failure_returns_no_cookie() {
        let user = PreparedAuthCookieUser::new(1, b"public", b"hash").unwrap();
        let generous = AuthCookieLimits::new(usize::MAX, usize::MAX);
        for (token, salt, limits, error) in [
            (
                b"".as_slice(),
                b"salt".as_slice(),
                generous,
                AuthCookieError::UnsupportedValue,
            ),
            (b"0", b"salt", generous, AuthCookieError::UnsupportedValue),
            (b"token", b"", generous, AuthCookieError::UnsupportedValue),
            (
                b"token",
                b"salt",
                AuthCookieLimits::new(0, usize::MAX),
                AuthCookieError::InputLimit,
            ),
            (
                b"token",
                b"salt",
                AuthCookieLimits::new(usize::MAX, 0),
                AuthCookieError::OutputLimit,
            ),
        ] {
            assert!(
                matches!(generate_with(&user, 0, token, salt, limits, |_, _, _, _, _| panic!("provider ran after refusal")), Err(observed) if observed == error)
            );
        }
        assert!(matches!(
            generate_with(
                &user,
                i64::MIN,
                b"token",
                b"salt",
                generous,
                |login, fragment, decimal, token, salt| {
                    assert_eq!(
                        (login, fragment, decimal, token, salt),
                        (
                            b"public".as_slice(),
                            b"hash".as_slice(),
                            b"-9223372036854775808".as_slice(),
                            b"token".as_slice(),
                            b"salt".as_slice()
                        )
                    );
                    Err(AuthCookieError::ProviderFailure)
                }
            ),
            Err(AuthCookieError::ProviderFailure)
        ));
    }
}

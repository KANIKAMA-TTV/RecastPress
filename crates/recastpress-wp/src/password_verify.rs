//! Verify-only WP-prefixed bcrypt subset; no login, generation or rehash policy.
//!
//! Raw password/hash Bytes, empty custom hasher/filter registry and user_id=0
//! are the reference profile. Only $wp$2y$ is admitted, not MD5, phpass, raw
//! bcrypt or Argon. No PHP casts, trim, UTF-8 sanitation or 72-byte C-string
//! compatibility is applied to passwords before their WP HMAC prehash.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use sha2::Sha384;
use std::fmt;
use zeroize::Zeroizing;

/// Explicit host internal allowance, never a default/product configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostMaxBcryptCost(u8);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidHostMaxBcryptCost;
impl fmt::Display for InvalidHostMaxBcryptCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("host bcrypt cost must be from 4 through 31")
    }
}
impl std::error::Error for InvalidHostMaxBcryptCost {}
impl HostMaxBcryptCost {
    pub fn new(value: u8) -> Result<Self, InvalidHostMaxBcryptCost> {
        if (4..=31).contains(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidHostMaxBcryptCost)
        }
    }
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Fixed classifications never contain a password, hash or provider message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WpBcryptVerifyError {
    UnsupportedHash,
    CostLimit,
    ProviderFailure,
}
impl fmt::Display for WpBcryptVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedHash => "password hash is outside the WP bcrypt subset",
            Self::CostLimit => "bcrypt cost exceeds the explicit host allowance",
            Self::ProviderFailure => "password verification provider failed",
        })
    }
}
impl std::error::Error for WpBcryptVerifyError {}

fn verify_with(
    password: &[u8],
    hash: &[u8],
    maximum: HostMaxBcryptCost,
    provider: impl FnOnce(&[u8], &str) -> Result<bool, ()>,
) -> Result<bool, WpBcryptVerifyError> {
    if hash.len() != 63 || !hash.is_ascii() || !hash.starts_with(b"$wp$2y$") || hash[9] != b'$' {
        return Err(WpBcryptVerifyError::UnsupportedHash);
    }
    // A >32-byte hash and empty custom hasher reach the source's password
    // length rejection before cost or algorithm processing. No HMAC/bcrypt.
    if password.len() > 4096 {
        return Ok(false);
    }
    if !hash[7..9].iter().all(u8::is_ascii_digit) {
        return Err(WpBcryptVerifyError::UnsupportedHash);
    }
    let cost = (hash[7] - b'0') * 10 + (hash[8] - b'0');
    if !(4..=31).contains(&cost) {
        return Err(WpBcryptVerifyError::UnsupportedHash);
    }
    if cost > maximum.get() {
        return Err(WpBcryptVerifyError::CostLimit);
    }
    let raw = std::str::from_utf8(&hash[3..]).map_err(|_| WpBcryptVerifyError::UnsupportedHash)?;
    // Parse only: bounded salt/checksum decoding, not bcrypt CPU work. This
    // also excludes noncanonical unused base64 bits from the supported domain.
    raw.parse::<bcrypt::HashParts>()
        .map_err(|_| WpBcryptVerifyError::UnsupportedHash)?;
    let encoded = wp_password_prehash(password)?;
    provider(&*encoded, raw).map_err(|()| WpBcryptVerifyError::ProviderFailure)
}

/// Shared fixed WP HMAC/standard-Base64 prehash. Callers decide whether the
/// source's trim step applies and enforce their own length/cost admission first.
pub(crate) fn wp_password_prehash(
    password: &[u8],
) -> Result<Zeroizing<[u8; 64]>, WpBcryptVerifyError> {
    let mut mac = Hmac::<Sha384>::new_from_slice(b"wp-sha384")
        .map_err(|_| WpBcryptVerifyError::ProviderFailure)?;
    mac.update(password);
    let digest = Zeroizing::new(<[u8; 48]>::from(mac.finalize().into_bytes()));
    let mut encoded = Zeroizing::new([0u8; 64]);
    let written = STANDARD
        .encode_slice(digest.as_slice(), encoded.as_mut_slice())
        .map_err(|_| WpBcryptVerifyError::ProviderFailure)?;
    if written != 64 {
        return Err(WpBcryptVerifyError::ProviderFailure);
    }
    Ok(encoded)
}

/// WordPress 7.1.3 pluggable.php:2843, explicit $wp$2y$/empty-hasher/empty-filter
/// verification only. true and false are match/mismatch; unsupported/cost/provider
/// errors are separate. The 4096 guard counts raw bytes and never trims input.
///
/// The host allowance is checked before HMAC/bcrypt work. It limits the encoded
/// bcrypt work factor, not wall time, concurrency, RSS or a manual product knob.
/// The caller must isolate bounded CPU work if later connected to async services.
/// Only the provider's fixed digest equality comparison is constant-time; hash
/// classification, cost/length branches and unknown-user login paths are not.
/// Derived 48/64-byte stack owners zeroize on drop, and bcrypt zeroizes its
/// pass buffer. Caller input, HMAC internals, compiler temporaries and allocator
/// memory are not guaranteed erased. No owned secret-bearing Debug/error data.
pub fn wp_check_password_wp_bcrypt_bytes(
    password: &[u8],
    hash: &[u8],
    maximum: HostMaxBcryptCost,
) -> Result<bool, WpBcryptVerifyError> {
    verify_with(password, hash, maximum, |prehash, raw| {
        bcrypt::verify(prehash, raw).map_err(|_| ())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shape(cost: &str) -> Vec<u8> {
        format!("$wp$2y${cost}$......................JXlH1Qf6S.wRMGfLjlHoYgbtH5EFLl.").into_bytes()
    }
    #[test]
    fn typed_host_cost_has_no_default_and_rejects_outside_provider_range() {
        for value in [4, 12, 31] {
            assert_eq!(HostMaxBcryptCost::new(value).unwrap().get(), value);
        }
        for value in [0, 1, 3, 32, 255] {
            assert_eq!(HostMaxBcryptCost::new(value), Err(InvalidHostMaxBcryptCost));
        }
    }
    #[test]
    fn length_guard_precedes_cost_and_provider_work_for_the_admitted_envelope() {
        let max = HostMaxBcryptCost::new(4).unwrap();
        assert_eq!(
            verify_with(&[0; 4097], &shape("31"), max, |_, _| panic!(
                "provider must not run"
            )),
            Ok(false)
        );
        for mut hash in [shape("xx"), shape("04")] {
            hash[31] = b'!';
            assert_eq!(
                verify_with(&[0; 4097], &hash, max, |_, _| panic!(
                    "provider must not run"
                )),
                Ok(false)
            );
        }
        assert_eq!(
            verify_with(b"dummy", &shape("31"), max, |_, _| panic!(
                "provider must not run"
            )),
            Err(WpBcryptVerifyError::CostLimit)
        );
        for hash in [b"legacy".to_vec(), shape("03"), shape("xx"), vec![0xff; 63]] {
            assert_eq!(
                verify_with(b"dummy", &hash, max, |_, _| panic!("provider must not run")),
                Err(WpBcryptVerifyError::UnsupportedHash)
            );
        }
    }
    #[test]
    fn derived_buffers_are_standard_base64_and_provider_failure_is_not_mismatch() {
        let max = HostMaxBcryptCost::new(4).unwrap();
        let hash = shape("04");
        assert_eq!(
            verify_with(b"dummy\0\xffpass", &hash, max, |prehash, raw| {
                assert_eq!(prehash.len(), 64);
                assert!(prehash.is_ascii());
                assert_eq!(
                    prehash,
                    b"t4vzBLJYQQr5TUoG4MEmtp0Y4nZeVFz+Il8umCnZYeSzL8LByS6sHx69rYwx1gYf"
                );
                assert_eq!(raw.len(), 60);
                Ok(false)
            }),
            Ok(false)
        );
        assert_eq!(
            verify_with(b"dummy\0\xff", &hash, max, |_, _| Err(())),
            Err(WpBcryptVerifyError::ProviderFailure)
        );
        for error in [
            WpBcryptVerifyError::UnsupportedHash,
            WpBcryptVerifyError::CostLimit,
            WpBcryptVerifyError::ProviderFailure,
        ] {
            assert!(!format!("{error} {error:?}").contains("dummy"));
            assert!(!format!("{error:?}").contains("$wp"));
        }
    }
}

//! Candidate default WP bcrypt generation with OS randomness, not an auth API.
//!
//! Fixed WP 7.1.3 pluggable.php:2753: empty custom hasher, empty algorithm/options
//! filters, cost12, raw length guard, PHP trim6, WP HMAC and explicit bcrypt TwoY.
//! No legacy hash, custom callback, rehash, login/session, HTTP or user lookup.

use crate::{HostMaxBcryptCost, password_verify::wp_password_prehash};
use std::fmt;
use zeroize::Zeroizing;

/// Opaque move-only WP storage value. No Display, mutable/owned escape or Clone.
/// Explicit caller copies through the borrowed accessor are outside this owner.
pub struct WpPasswordHash {
    bytes: Zeroizing<[u8; 63]>,
}
impl WpPasswordHash {
    pub fn as_wp_storage_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
impl fmt::Debug for WpPasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WpPasswordHash([redacted])")
    }
}

/// The source's '*' sentinel is a typed outcome, never an error or a valid hash.
#[derive(Debug)]
pub enum WpPasswordHashOutcome {
    Hash(WpPasswordHash),
    PasswordTooLong,
}
impl WpPasswordHashOutcome {
    /// Explicit source-storage representation, including '*' for overlength.
    /// This does not authorize storing the marker as a usable credential.
    pub fn as_wp_storage_bytes(&self) -> &[u8] {
        match self {
            Self::Hash(hash) => hash.as_wp_storage_bytes(),
            Self::PasswordTooLong => b"*",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WpPasswordHashError {
    CostLimit,
    EntropyUnavailable,
    ProviderFailure,
}
impl fmt::Display for WpPasswordHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CostLimit => "default bcrypt cost exceeds the explicit host allowance",
            Self::EntropyUnavailable => "OS entropy is unavailable for password hashing",
            Self::ProviderFailure => "password hashing provider failed",
        })
    }
}
impl std::error::Error for WpPasswordHashError {}

#[derive(Clone, Copy)]
enum ProviderError {
    EntropyUnavailable,
    Failure,
}
fn classify(error: bcrypt::BcryptError) -> ProviderError {
    match error {
        bcrypt::BcryptError::Rand(_) => ProviderError::EntropyUnavailable,
        _ => ProviderError::Failure,
    }
}
fn php_trim6(input: &[u8]) -> &[u8] {
    fn trimmed(byte: &u8) -> bool {
        matches!(*byte, 0 | 9 | 10 | 11 | 13 | 32)
    }
    let start = input
        .iter()
        .position(|byte| !trimmed(byte))
        .unwrap_or(input.len());
    let rest = &input[start..];
    let end = rest
        .iter()
        .rposition(|byte| !trimmed(byte))
        .map_or(0, |index| index + 1);
    &rest[..end]
}
fn generate_with(
    password: &[u8],
    maximum: HostMaxBcryptCost,
    provider: impl FnOnce(&[u8], u32) -> Result<String, ProviderError>,
) -> Result<WpPasswordHashOutcome, WpPasswordHashError> {
    // Source order: the raw-byte guard precedes trim, host refusal, HMAC and RNG.
    if password.len() > 4096 {
        return Ok(WpPasswordHashOutcome::PasswordTooLong);
    }
    if maximum.get() < 12 {
        return Err(WpPasswordHashError::CostLimit);
    }
    let prehash = wp_password_prehash(php_trim6(password))
        .map_err(|_| WpPasswordHashError::ProviderFailure)?;
    let raw = Zeroizing::new(
        provider(prehash.as_slice(), 12).map_err(|error| match error {
            ProviderError::EntropyUnavailable => WpPasswordHashError::EntropyUnavailable,
            ProviderError::Failure => WpPasswordHashError::ProviderFailure,
        })?,
    );
    if raw.len() != 60 || !raw.starts_with("$2y$12$") {
        return Err(WpPasswordHashError::ProviderFailure);
    }
    let parsed = raw
        .parse::<bcrypt::HashParts>()
        .map_err(|_| WpPasswordHashError::ProviderFailure)?;
    if parsed.get_cost() != 12 {
        return Err(WpPasswordHashError::ProviderFailure);
    }
    let mut bytes = Zeroizing::new([0u8; 63]);
    bytes[..3].copy_from_slice(b"$wp");
    bytes[3..].copy_from_slice(raw.as_bytes());
    Ok(WpPasswordHashOutcome::Hash(WpPasswordHash { bytes }))
}

/// Default WP bcrypt generation candidate for Bytes and empty callbacks.
/// Raw >4096 returns PasswordTooLong/source '*', even when host maximum <12.
/// Otherwise cost12 must be allowed, then trim only space/TAB/LF/CR/NUL/VT,
/// HMAC-SHA384 and standard Base64, and request an OS-random salt from bcrypt.
/// No entropy fallback, seeded generator, supplied salt or alternative algorithm.
///
/// Derived 48/64-byte owners and final hash owner zeroize; bcrypt wipes its pass
/// buffer. Caller input, HMAC/provider internals, compiler copies and allocator
/// memory are not guaranteed erased. Provider formatting allocations are unpooled;
/// allocation aborts are not converted into a complete-memory/availability bound.
/// Host maximum is explicit internal admission, not a default product cost policy
/// or wall-time/concurrency limit. Random hashes require cross-verification, not
/// byte-equal differential output or an added deterministic registry API count.
pub fn wp_hash_password_default_bcrypt_bytes(
    password: &[u8],
    maximum: HostMaxBcryptCost,
) -> Result<WpPasswordHashOutcome, WpPasswordHashError> {
    generate_with(password, maximum, |prehash, cost| {
        bcrypt::hash_with_result(prehash, cost)
            .map(|parts| parts.format_for_version(bcrypt::Version::TwoY))
            .map_err(classify)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: &str = "$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C";
    #[test]
    fn raw_length_then_cost_refuse_without_calling_provider_or_entropy() {
        let low = HostMaxBcryptCost::new(4).unwrap();
        for password in [vec![b' '; 4097], vec![0; 4097]] {
            let out =
                generate_with(&password, low, |_, _| panic!("provider must not run")).unwrap();
            assert!(matches!(out, WpPasswordHashOutcome::PasswordTooLong));
            assert_eq!(out.as_wp_storage_bytes(), b"*");
        }
        assert!(matches!(
            generate_with(&[b' '; 4096], low, |_, _| panic!("provider must not run")),
            Err(WpPasswordHashError::CostLimit)
        ));
    }
    #[test]
    fn trim_is_exactly_six_php_bytes_and_preserves_interior_ff_unicode_and_invalid_bytes() {
        for byte in [0, 9, 10, 11, 13, 32] {
            assert_eq!(php_trim6(&[byte, b'x', byte]), b"x");
        }
        assert_eq!(php_trim6(b" \t\0x\0\xff\x0c\r\n"), b"x\0\xff\x0c");
        assert_eq!(
            php_trim6("\u{a0}x\u{a0}".as_bytes()),
            "\u{a0}x\u{a0}".as_bytes()
        );
        assert_eq!(php_trim6(b"\x0cx\x0c"), b"\x0cx\x0c");
        assert_eq!(php_trim6(b" \0\t\r\n\x0b"), b"");
    }
    #[test]
    fn fixed_prehash_twelve_and_twoy_owner_are_redacted_without_exposing_mutable_hash() {
        let out = generate_with(
            b" \t\0dummy-pass\r\n\x0b",
            HostMaxBcryptCost::new(12).unwrap(),
            |prehash, cost| {
                assert_eq!(cost, 12);
                assert_eq!(
                    prehash,
                    b"SEJUqwkTPkl4h16M/HrElCvXODlWk9QGbp/m6nXbQepg/cOcyCj3yPZRKCxecv3C"
                );
                Ok(RAW.into())
            },
        )
        .unwrap();
        assert_eq!(out.as_wp_storage_bytes(), format!("$wp{RAW}").as_bytes());
        assert!(!format!("{out:?}").contains(RAW));
        assert!(!format!("{out:?}").contains("dummy-pass"));
        assert!(matches!(out, WpPasswordHashOutcome::Hash(_)));
    }
    #[test]
    fn entropy_and_other_provider_failures_are_closed_and_no_fallback_is_called() {
        let max = HostMaxBcryptCost::new(12).unwrap();
        for (provider, error) in [
            (
                ProviderError::EntropyUnavailable,
                WpPasswordHashError::EntropyUnavailable,
            ),
            (ProviderError::Failure, WpPasswordHashError::ProviderFailure),
        ] {
            assert!(
                matches!(generate_with(b"dummy",max,|_,_|Err(provider)),Err(found)if found==error)
            );
            assert!(!format!("{error} {error:?}").contains("dummy"));
        }
        for invalid in [
            "malformed",
            "$2b$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C",
            "$2y$04$......................p4I2i5y/qZ1mBr4uEKqha7UHXK0Clmq",
        ] {
            assert!(matches!(
                generate_with(b"dummy", max, |_, _| Ok(invalid.into())),
                Err(WpPasswordHashError::ProviderFailure)
            ));
        }
        assert!(matches!(
            classify(bcrypt::BcryptError::CostNotAllowed(0)),
            ProviderError::Failure
        ));
    }
}

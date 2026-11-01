//! Raw wp_check_password dispatcher, explicit empty callbacks and user_id=0.
//!
//! Source order is hash-length MD5, raw-password length, then the selected
//! WP bcrypt/portable subsets. No coercion, trim, crypt fallback or auth policy.

use crate::{
    HostMaxBcryptCost, HostMaxPhpassCost, LegacyPasswordVerifyError, WpBcryptVerifyError,
    wp_check_password_legacy_md5_bytes, wp_check_password_phpass_portable_bytes,
    wp_check_password_wp_bcrypt_bytes,
};
use std::fmt;

/// Both stored-work-factor allowances are mandatory, with no product default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostPasswordWorkLimits {
    bcrypt: HostMaxBcryptCost,
    portable: HostMaxPhpassCost,
}
impl HostPasswordWorkLimits {
    pub fn new(bcrypt: HostMaxBcryptCost, portable: HostMaxPhpassCost) -> Self {
        Self { bcrypt, portable }
    }
    pub fn bcrypt_maximum(self) -> HostMaxBcryptCost {
        self.bcrypt
    }
    pub fn portable_maximum(self) -> HostMaxPhpassCost {
        self.portable
    }
}

/// Fixed classifications never contain a password, hash or provider message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasswordDispatchError {
    UnsupportedHash,
    BcryptCostLimit,
    PortableCostLimit,
    ProviderFailure,
}
impl fmt::Display for PasswordDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedHash => "password hash is outside the selected verification subsets",
            Self::BcryptCostLimit => "bcrypt cost exceeds the explicit host allowance",
            Self::PortableCostLimit => "portable phpass cost exceeds the explicit host allowance",
            Self::ProviderFailure => "password verification provider failed",
        })
    }
}
impl std::error::Error for PasswordDispatchError {}

fn dispatch_with(
    password: &[u8],
    hash: &[u8],
    limits: HostPasswordWorkLimits,
    md5: impl FnOnce(&[u8], &[u8]) -> Result<bool, PasswordDispatchError>,
    bcrypt: impl FnOnce(&[u8], &[u8], HostMaxBcryptCost) -> Result<bool, PasswordDispatchError>,
    portable: impl FnOnce(&[u8], &[u8], HostMaxPhpassCost) -> Result<bool, PasswordDispatchError>,
) -> Result<bool, PasswordDispatchError> {
    if hash.len() <= 32 {
        return md5(password, hash);
    }
    // Empty custom hasher profile: reject raw overlength before inspecting
    // prefix, ASCII, cost, salt, checksum or unsupported algorithm syntax.
    if password.len() > 4096 {
        return Ok(false);
    }
    if hash.starts_with(b"$wp") {
        return bcrypt(password, hash, limits.bcrypt);
    }
    if hash.starts_with(b"$P$") {
        return portable(password, hash, limits.portable);
    }
    Err(PasswordDispatchError::UnsupportedHash)
}

fn bcrypt_error(error: WpBcryptVerifyError) -> PasswordDispatchError {
    match error {
        WpBcryptVerifyError::UnsupportedHash => PasswordDispatchError::UnsupportedHash,
        WpBcryptVerifyError::CostLimit => PasswordDispatchError::BcryptCostLimit,
        WpBcryptVerifyError::ProviderFailure => PasswordDispatchError::ProviderFailure,
    }
}
fn legacy_error(error: LegacyPasswordVerifyError) -> PasswordDispatchError {
    match error {
        LegacyPasswordVerifyError::UnsupportedHash => PasswordDispatchError::UnsupportedHash,
        LegacyPasswordVerifyError::CostLimit => PasswordDispatchError::PortableCostLimit,
    }
}

/// Fixed WP 7.1.3 pluggable.php:2843, raw Bytes, empty hasher/filter registry,
/// user_id=0. Match/mismatch is separate from subset, host-cost and provider
/// errors. This does not execute filters or implement their bookkeeping.
///
/// Hash length <=32 always selects the MD5 helper, even for short PHP-looking
/// prefixes or passwords longer than 4096 bytes. For all other hashes, a raw
/// password longer than 4096 returns false before any format parsing/delegation.
/// Otherwise $wp delegates once to the $wp$2y$ helper, $P$ once to canonical
/// portable phpass, and other formats are unsupported. No fallback is attempted.
/// Existing standalone helpers retain their narrower envelope admission order.
///
/// No trim, UTF-8/NUL conversion, RNG, new digest primitive or payload copy.
/// Helper zeroization scopes and unbounded legacy MD5 input work still apply.
/// Explicit host costs are not wall-time/concurrency, process memory, credential
/// migration, unknown-user timing, login, session or authorization guarantees.
pub fn wp_check_password_raw_empty_filters_bytes(
    password: &[u8],
    hash: &[u8],
    limits: HostPasswordWorkLimits,
) -> Result<bool, PasswordDispatchError> {
    dispatch_with(
        password,
        hash,
        limits,
        |password, hash| wp_check_password_legacy_md5_bytes(password, hash).map_err(legacy_error),
        |password, hash, maximum| {
            wp_check_password_wp_bcrypt_bytes(password, hash, maximum).map_err(bcrypt_error)
        },
        |password, hash, maximum| {
            wp_check_password_phpass_portable_bytes(password, hash, maximum).map_err(legacy_error)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn limits() -> HostPasswordWorkLimits {
        HostPasswordWorkLimits::new(
            HostMaxBcryptCost::new(4).unwrap(),
            HostMaxPhpassCost::new(7).unwrap(),
        )
    }

    #[test]
    fn every_selected_branch_delegates_exactly_once_with_unchanged_bytes_and_typed_limits() {
        for (hash, expected) in [
            (b"$P$".as_slice(), 0),
            (b"$wp12345678901234567890123456789012", 1),
            (b"$P$12345678901234567890123456789012", 2),
        ] {
            let calls = [const { Cell::new(0) }; 3];
            let password = b" \0\xffdummy ";
            let result = dispatch_with(
                password,
                hash,
                limits(),
                |raw, stored| {
                    assert_eq!(raw, password);
                    assert_eq!(stored, hash);
                    calls[0].set(calls[0].get() + 1);
                    Ok(true)
                },
                |raw, stored, maximum| {
                    assert_eq!(raw, password);
                    assert_eq!(stored, hash);
                    assert_eq!(maximum.get(), 4);
                    calls[1].set(calls[1].get() + 1);
                    Ok(true)
                },
                |raw, stored, maximum| {
                    assert_eq!(raw, password);
                    assert_eq!(stored, hash);
                    assert_eq!(maximum.get(), 7);
                    calls[2].set(calls[2].get() + 1);
                    Ok(true)
                },
            );
            assert_eq!(result, Ok(true));
            for (index, count) in calls.iter().enumerate() {
                assert_eq!(count.get(), u8::from(index == expected));
            }
        }
    }

    #[test]
    fn long_raw_password_is_guarded_before_any_format_parse_or_helper_except_the_md5_branch() {
        for hash in [
            vec![255; 33],
            b"$wpbad".repeat(6),
            b"$P$bad".repeat(6),
            b"$2y$not-admitted".repeat(3),
        ] {
            assert_eq!(
                dispatch_with(
                    &[0; 4097],
                    &hash,
                    limits(),
                    |_, _| panic!("MD5 must not run"),
                    |_, _, _| panic!("bcrypt must not run"),
                    |_, _, _| panic!("portable must not run")
                ),
                Ok(false)
            );
        }
        let calls = Cell::new(0);
        assert_eq!(
            dispatch_with(
                &[0; 4097],
                &[255; 32],
                limits(),
                |raw, hash| {
                    assert_eq!(raw.len(), 4097);
                    assert_eq!(hash.len(), 32);
                    calls.set(calls.get() + 1);
                    Ok(false)
                },
                |_, _, _| panic!("bcrypt must not run"),
                |_, _, _| panic!("portable must not run")
            ),
            Ok(false)
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn provider_errors_are_not_mismatch_or_fallback_and_classifications_remain_closed() {
        let bcrypt_hash = b"$wp12345678901234567890123456789012";
        assert_eq!(
            dispatch_with(
                b"dummy",
                bcrypt_hash,
                limits(),
                |_, _| panic!("MD5 must not run"),
                |_, _, _| Err(PasswordDispatchError::ProviderFailure),
                |_, _, _| panic!("no fallback")
            ),
            Err(PasswordDispatchError::ProviderFailure)
        );
        assert_eq!(
            bcrypt_error(WpBcryptVerifyError::CostLimit),
            PasswordDispatchError::BcryptCostLimit
        );
        assert_eq!(
            legacy_error(LegacyPasswordVerifyError::CostLimit),
            PasswordDispatchError::PortableCostLimit
        );
        for error in [
            PasswordDispatchError::UnsupportedHash,
            PasswordDispatchError::BcryptCostLimit,
            PasswordDispatchError::PortableCostLimit,
            PasswordDispatchError::ProviderFailure,
        ] {
            assert!(!format!("{error:?} {error}").contains("dummy"));
        }
    }
}

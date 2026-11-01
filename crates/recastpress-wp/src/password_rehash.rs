//! Default-bcrypt metadata hint with empty custom hasher and three empty filters.
//!
//! Fixed WP pluggable.php:2911 / isolated PHP 8.5.12, Bytes and user_id=0.
//! Non-$wp input is always true. Prefixed input is deliberately narrower than
//! PHP's loose builtin recognition: only the existing verify kernel's canonical
//! $wp$2y$ 63-byte/cost4..31 domain is admitted. No KDF, RNG, trim, password input,
//! 4096-byte limit, host work allowance, hook bookkeeping or write/login policy.

use crate::CompatibilityError;

/// Pure metadata predicate, not proof that a hash or password is valid/safe.
/// Uses the pinned provider's bounded parse only, never bcrypt computation.
/// Parser-owned salt/checksum allocations are not a pooled/RSS accounting claim.
pub fn wp_password_needs_rehash_default_bcrypt_bytes(
    hash: &[u8],
) -> Result<bool, CompatibilityError> {
    if !hash.starts_with(b"$wp") {
        return Ok(true);
    }
    if hash.len() != 63
        || !hash.is_ascii()
        || !hash.starts_with(b"$wp$2y$")
        || hash[9] != b'$'
        || !hash[7..9].iter().all(u8::is_ascii_digit)
    {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let cost = (hash[7] - b'0') * 10 + hash[8] - b'0';
    if !(4..=31).contains(&cost) {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let raw = std::str::from_utf8(&hash[3..]).map_err(|_| CompatibilityError::UnsupportedValue)?;
    // Same canonical salt/checksum parser as password_verify, without prehash,
    // secret comparison or a computation whose cost scales with this metadata.
    raw.parse::<bcrypt::HashParts>()
        .map_err(|_| CompatibilityError::UnsupportedValue)?;
    Ok(cost != 12)
}

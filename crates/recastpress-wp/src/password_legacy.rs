//! Verify-only legacy WP MD5 and canonical portable phpass subsets.
//!
//! Fixed pluggable.php:2843 / class-phpass.php:131,247, raw Bytes, empty custom
//! hasher/filter registry and user_id=0. No generation, crypt fallback, login,
//! session, rehash policy or changes to the separate WP bcrypt APIs.

use md5::{Digest, Md5};
use std::fmt;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const ITOA64: &[u8; 64] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Required internal host allowance for the stored portable work factor.
/// No default, product setting, wall-time guarantee or concurrency policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostMaxPhpassCost(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidHostMaxPhpassCost;

impl fmt::Display for InvalidHostMaxPhpassCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("host portable phpass cost must be from 7 through 30")
    }
}
impl std::error::Error for InvalidHostMaxPhpassCost {}

impl HostMaxPhpassCost {
    pub fn new(value: u8) -> Result<Self, InvalidHostMaxPhpassCost> {
        if (7..=30).contains(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidHostMaxPhpassCost)
        }
    }
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Fixed diagnostics contain no password, hash, salt or provider text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyPasswordVerifyError {
    UnsupportedHash,
    CostLimit,
}
impl fmt::Display for LegacyPasswordVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedHash => "password hash is outside the selected legacy subset",
            Self::CostLimit => "portable phpass cost exceeds the explicit host allowance",
        })
    }
}
impl std::error::Error for LegacyPasswordVerifyError {}

fn digest_parts(parts: &[&[u8]]) -> Zeroizing<[u8; 16]> {
    let mut hasher = Md5::new();
    for part in parts {
        hasher.update(part);
    }
    Zeroizing::new(hasher.finalize().into())
}

fn md5_with(
    password: &[u8],
    hash: &[u8],
    digest: impl FnOnce(&[u8]) -> Zeroizing<[u8; 16]>,
) -> Result<bool, LegacyPasswordVerifyError> {
    if hash.len() > 32 {
        return Err(LegacyPasswordVerifyError::UnsupportedHash);
    }
    // The source's lowercase hexadecimal MD5 is always exactly 32 bytes.
    if hash.len() < 32 {
        return Ok(false);
    }
    let bytes = digest(password);
    let mut encoded = Zeroizing::new([0u8; 32]);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (byte, pair) in bytes.iter().zip(encoded.chunks_exact_mut(2)) {
        pair[0] = HEX[usize::from(byte >> 4)];
        pair[1] = HEX[usize::from(byte & 15)];
    }
    Ok(bool::from(encoded.as_slice().ct_eq(hash)))
}

/// The source's first legacy branch, hash length <=32, before its 4096 guard.
/// Hashes shorter than 32 always mismatch; full hashes compare against lowercase
/// hexadecimal MD5 of every raw password byte. No trim/UTF-8/NUL transformation.
/// There is deliberately no invented 4096-byte password limit for this branch.
/// This is historical verification only, never a new-credential hash generator.
pub fn wp_check_password_legacy_md5_bytes(
    password: &[u8],
    hash: &[u8],
) -> Result<bool, LegacyPasswordVerifyError> {
    md5_with(password, hash, |raw| digest_parts(&[raw]))
}

fn portable_digest(password: &[u8], salt: &[u8], cost: u8) -> Zeroizing<[u8; 16]> {
    let mut digest = digest_parts(&[salt, password]);
    for _ in 0..(1u32 << cost) {
        digest = digest_parts(&[digest.as_slice(), password]);
    }
    digest
}

// phpass's little-endian 6-bit alphabet encoding, not standard/bcrypt Base64.
// The MD5 primitive itself is provided by md-5; it is not reimplemented here.
fn encode_portable_digest(digest: &[u8; 16], output: &mut [u8]) {
    debug_assert_eq!(output.len(), 22);
    let mut bits = 0u32;
    let mut available = 0u8;
    let mut written = 0;
    for byte in digest {
        bits |= u32::from(*byte) << available;
        available += 8;
        while available >= 6 {
            output[written] = ITOA64[(bits & 63) as usize];
            written += 1;
            bits >>= 6;
            available -= 6;
        }
    }
    if available != 0 {
        output[written] = ITOA64[bits as usize];
    }
}

fn portable_with(
    password: &[u8],
    hash: &[u8],
    maximum: HostMaxPhpassCost,
    digest: impl FnOnce(&[u8], &[u8], u8) -> Zeroizing<[u8; 16]>,
) -> Result<bool, LegacyPasswordVerifyError> {
    if hash.len() != 34 || !hash.starts_with(b"$P$") {
        return Err(LegacyPasswordVerifyError::UnsupportedHash);
    }
    // An admitted >32-byte hash reaches the source length guard before branch,
    // cost, salt or checksum parsing. Malformed bodies still return false here.
    if password.len() > 4096 {
        return Ok(false);
    }
    let cost = ITOA64
        .iter()
        .position(|byte| *byte == hash[3])
        .filter(|cost| (7..=30).contains(cost))
        .ok_or(LegacyPasswordVerifyError::UnsupportedHash)? as u8;
    if cost > maximum.get() {
        return Err(LegacyPasswordVerifyError::CostLimit);
    }
    // Explicit initial subset: crypt64 ASCII salt, canonical 128-bit checksum.
    // The PHP source accepts arbitrary eight-byte salts; those are not claimed.
    if !hash[4..].iter().all(|byte| ITOA64.contains(byte)) || !b"./01".contains(&hash[33]) {
        return Err(LegacyPasswordVerifyError::UnsupportedHash);
    }
    let digest = digest(password, &hash[4..12], cost);
    let mut encoded = Zeroizing::new([0u8; 34]);
    encoded[..12].copy_from_slice(&hash[..12]);
    encode_portable_digest(&digest, &mut encoded[12..]);
    // Compare the complete re-encoded value. Decoding only the digest could
    // accidentally ignore noncanonical unused bits in the final stored symbol.
    Ok(bool::from(encoded.as_slice().ct_eq(hash)))
}

/// Canonical $P$ portable phpass verification, empty callbacks/user_id=0.
/// Admission: exact 34-byte $P$ envelope, then raw >4096 false, valid cost7..30,
/// host maximum, crypt64 salt/checksum and canonical final symbol, then MD5.
/// Each admitted computation performs 1 + 2^cost MD5 calls, over all raw bytes.
/// No fallback to crypt, $H$, arbitrary salt bytes, trim, generation or rehash.
///
/// The fixed MD5 provider zeroizes its state with its enabled zeroize feature;
/// our 16-byte digest and 34-byte encoding owners zeroize on drop. Caller input,
/// provider temporaries, compiler copies and all process memory are not covered.
/// Fixed-size scratch is unpooled. Host cost admission is not CPU cancellation,
/// full-RSS accounting, a recommended product setting or an auth timing claim.
pub fn wp_check_password_phpass_portable_bytes(
    password: &[u8],
    hash: &[u8],
    maximum: HostMaxPhpassCost,
) -> Result<bool, LegacyPasswordVerifyError> {
    portable_with(password, hash, maximum, portable_digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(cost: u8) -> [u8; 34] {
        let mut hash = [b'.'; 34];
        hash[..3].copy_from_slice(b"$P$");
        hash[3] = ITOA64[usize::from(cost)];
        hash
    }

    #[test]
    fn typed_cost_has_no_default_and_only_accepts_portable_seven_through_thirty() {
        for cost in [7, 13, 30] {
            assert_eq!(HostMaxPhpassCost::new(cost).unwrap().get(), cost);
        }
        for cost in [0, 4, 6, 31, 63, 255] {
            assert_eq!(HostMaxPhpassCost::new(cost), Err(InvalidHostMaxPhpassCost));
        }
    }

    #[test]
    fn guards_never_call_md5_and_preserve_length_cost_and_canonical_priority() {
        let max = HostMaxPhpassCost::new(7).unwrap();
        let no_cpu = |_: &[u8], _: &[u8], _: u8| panic!("MD5 must not run");
        let mut malformed = shape(13);
        malformed[4] = 255;
        malformed[33] = b'z';
        assert_eq!(
            portable_with(&[0; 4097], &malformed, max, no_cpu),
            Ok(false)
        );
        assert_eq!(
            portable_with(b"dummy", &malformed, max, no_cpu),
            Err(LegacyPasswordVerifyError::CostLimit)
        );
        malformed[3] = b'!';
        assert_eq!(
            portable_with(&[0; 4097], &malformed, max, no_cpu),
            Ok(false)
        );
        assert_eq!(
            portable_with(b"dummy", &malformed, max, no_cpu),
            Err(LegacyPasswordVerifyError::UnsupportedHash)
        );
        assert_eq!(
            portable_with(&[0; 4097], &malformed[..33], max, no_cpu),
            Err(LegacyPasswordVerifyError::UnsupportedHash)
        );
        for index in [4, 12, 33] {
            let mut malformed = shape(7);
            malformed[index] = if index == 33 { b'2' } else { 255 };
            assert_eq!(
                portable_with(b"dummy", &malformed, max, no_cpu),
                Err(LegacyPasswordVerifyError::UnsupportedHash)
            );
        }
        for len in 0..32 {
            assert_eq!(
                md5_with(b"dummy", &vec![b'0'; len], |_| panic!("MD5 must not run")),
                Ok(false)
            );
        }
        assert_eq!(
            md5_with(b"dummy", &[b'0'; 33], |_| panic!("MD5 must not run")),
            Err(LegacyPasswordVerifyError::UnsupportedHash)
        );
    }

    #[test]
    fn encoding_is_little_endian_and_compares_complete_canonical_storage() {
        let mut encoded = [0u8; 22];
        encode_portable_digest(&[0; 16], &mut encoded);
        assert_eq!(&encoded, b"......................");
        encode_portable_digest(&[255; 16], &mut encoded);
        assert_eq!(&encoded, b"zzzzzzzzzzzzzzzzzzzzz1");
        let max = HostMaxPhpassCost::new(7).unwrap();
        let hash = shape(7);
        assert_eq!(
            portable_with(b"dummy", &hash, max, |raw, salt, cost| {
                assert_eq!(raw, b"dummy");
                assert_eq!(salt, b"........");
                assert_eq!(cost, 7);
                Zeroizing::new([0; 16])
            }),
            Ok(true)
        );
        assert_eq!(
            portable_with(b"dummy", &hash, max, |_, _, _| Zeroizing::new([255; 16])),
            Ok(false)
        );
        for error in [
            LegacyPasswordVerifyError::UnsupportedHash,
            LegacyPasswordVerifyError::CostLimit,
        ] {
            assert!(!format!("{error:?} {error}").contains("dummy"));
        }
    }
}

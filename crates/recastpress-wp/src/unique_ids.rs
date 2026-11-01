//! Byte-prefix subsets of WordPress 7.1.3 functions.php:8156 and :8175.
//!
//! WordPress uses function-local statics. Here the host explicitly owns one
//! context per chosen request/execution lifetime; no process-global state or
//! Clone is provided. Starting another context restarts both counters. IDs are
//! neither universally unique nor security tokens, and prefix concatenation can
//! collide. Nonstring coercions/diagnostics, PHP float-counter overflow, host
//! budget integration and Wasm execution are outside this candidate API.

use crate::CompatibilityError;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct UniqueIds {
    counter: i64,
    prefixed: BTreeMap<Vec<u8>, i64>,
}

impl UniqueIds {
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses one counter across every byte prefix in this context.
    /// At signed 64-bit overflow, returns UnsupportedValue without incrementing.
    pub fn wp_unique_id(&mut self, prefix: &[u8]) -> Result<Vec<u8>, CompatibilityError> {
        let next = self
            .counter
            .checked_add(1)
            .ok_or(CompatibilityError::UnsupportedValue)?;
        let result = concatenate(prefix, next);
        self.counter = next;
        Ok(result)
    }

    /// Uses one counter per exact byte prefix, independent of wp_unique_id.
    /// Case, empty/numeric prefixes, NUL and non-UTF8 bytes remain distinct.
    /// Nonstring inputs and PHP's diagnostic fallback to empty are not modeled.
    pub fn wp_unique_prefixed_id(&mut self, prefix: &[u8]) -> Result<Vec<u8>, CompatibilityError> {
        let current = self.prefixed.get(prefix).copied().unwrap_or(0);
        let next = current
            .checked_add(1)
            .ok_or(CompatibilityError::UnsupportedValue)?;
        let result = concatenate(prefix, next);
        self.prefixed.insert(prefix.to_vec(), next);
        Ok(result)
    }
}

fn concatenate(prefix: &[u8], counter: i64) -> Vec<u8> {
    let mut output = prefix.to_vec();
    output.extend_from_slice(counter.to_string().as_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_id_overflow_preserves_both_private_counter_domains_and_last_decimal() {
        let mut ids = UniqueIds::new();
        ids.counter = i64::MAX - 1;
        ids.prefixed.insert(b"full\0\xff".to_vec(), i64::MAX - 1);
        ids.prefixed.insert(b"other".to_vec(), 7);
        assert_eq!(ids.wp_unique_id(b"g-").unwrap(), b"g-9223372036854775807");
        assert_eq!(ids.counter, i64::MAX);
        assert_eq!(
            ids.wp_unique_prefixed_id(b"full\0\xff").unwrap(),
            b"full\0\xff9223372036854775807"
        );
        assert_eq!(ids.prefixed.get(b"full\0\xff".as_slice()), Some(&i64::MAX));
        let prefixed_before = ids.prefixed.clone();
        assert_eq!(
            ids.wp_unique_id(b"different"),
            Err(CompatibilityError::UnsupportedValue)
        );
        assert_eq!(
            ids.wp_unique_prefixed_id(b"full\0\xff"),
            Err(CompatibilityError::UnsupportedValue)
        );
        assert_eq!(ids.counter, i64::MAX);
        assert_eq!(ids.prefixed, prefixed_before);
        assert_eq!(ids.wp_unique_prefixed_id(b"other").unwrap(), b"other8");
        assert_eq!(ids.wp_unique_prefixed_id(b"fresh").unwrap(), b"fresh1");
        assert_eq!(ids.counter, i64::MAX);
        assert_eq!(ids.prefixed.get(b"full\0\xff".as_slice()), Some(&i64::MAX));
    }
}

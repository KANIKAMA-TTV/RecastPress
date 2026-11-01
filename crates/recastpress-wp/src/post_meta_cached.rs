//! Selected get_post_meta(..., true) return semantics from the pinned post/meta
//! helpers, for an explicitly caller-prepared COMPLETE, NONEMPTY post-meta cache.
//! The caller attests that get_post_metadata/default_post_metadata and `all`
//! filters are absent. This is not a cache population, DB lookup or hook API.
//! The cache's association with the positive object ID is a caller contract,
//! not a verified database identity or an authorization/principal capability.

use crate::{
    serialization::{
        LimitedUnserializeError, UnserializeLimits, maybe_unserialize_bytes_with_limits,
    },
    value::{PhpArray, PhpKey, PhpValue},
};

/// Borrowed cache view. Only ID positivity and nonempty cache truthiness are
/// validated here. The getter validates the selected raw value-list shape;
/// unrelated keys are neither interpreted nor cloned. Payload Debug is hidden.
pub struct PreparedPostMetaCache<'a> {
    cache: &'a PhpArray,
}

impl<'a> PreparedPostMetaCache<'a> {
    pub fn new(object_id: i64, cache: &'a PhpArray) -> Result<Self, PostMetaCachedError> {
        if object_id <= 0 || cache.is_empty() {
            return Err(PostMetaCachedError::UnsupportedValue);
        }
        Ok(Self { cache })
    }
}

impl std::fmt::Debug for PreparedPostMetaCache<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedPostMetaCache([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostMetaCachedError {
    UnsupportedValue,
    Decode(LimitedUnserializeError),
}

impl std::fmt::Display for PostMetaCachedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedValue => "unsupported cached post metadata request",
            Self::Decode(_) => "cached post metadata decoding failed",
        })
    }
}

impl std::error::Error for PostMetaCachedError {}

/// Return one selected value using a complete nonempty cache and no relevant
/// filters. Empty and `0` byte keys request whole-cache behavior in PHP and are
/// outside this SINGLE profile. PHP integer-string key conversion is preserved
/// without key cloning/formatting. The selected entry must be a nonempty dense
/// raw list of Null/Bytes; only its first value is decoded. Other cache entries
/// and later serialized values are not decoded or charged to the decode limits.
///
/// A missing/isset-false entry, SQL NULL at index 0, or decoded serialized Null
/// follows get_metadata's default-single path to an empty byte string. Serialized
/// false remains Bool(false). Unsupported encodings/limits remain explicit errors.
///
/// The four limits apply to the selected byte value only. Existing cache storage,
/// lookup/key/list-validation work, returned empty defaults, node/capacity/RSS and
/// shared budgets are not covered. No object activation, cache miss fallback,
/// multiple/full-cache result, filters, status policy or authorization is added.
/// Returned PhpValue Debug may expose data; callers must keep returned values private.
pub fn get_post_meta_single_cached_no_filters(
    cache: &PreparedPostMetaCache<'_>,
    key: &[u8],
    limits: UnserializeLimits,
) -> Result<PhpValue, PostMetaCachedError> {
    if key.is_empty() || key == b"0" {
        return Err(PostMetaCachedError::UnsupportedValue);
    }
    let values = match cache.cache.get_bytes(key) {
        None | Some(PhpValue::Null) => return Ok(PhpValue::Bytes(Vec::new())),
        Some(PhpValue::Array(values)) if !values.is_empty() => values,
        _ => return Err(PostMetaCachedError::UnsupportedValue),
    };
    for (index, (key, value)) in values.iter().enumerate() {
        let expected = i64::try_from(index).map_err(|_| PostMetaCachedError::UnsupportedValue)?;
        if key != &PhpKey::Integer(expected)
            || !matches!(value, PhpValue::Null | PhpValue::Bytes(_))
        {
            return Err(PostMetaCachedError::UnsupportedValue);
        }
    }
    let first = values
        .iter()
        .next()
        .ok_or(PostMetaCachedError::UnsupportedValue)?
        .1;
    let result = match first {
        PhpValue::Null => return Ok(PhpValue::Bytes(Vec::new())),
        PhpValue::Bytes(raw) => {
            maybe_unserialize_bytes_with_limits(raw, limits).map_err(PostMetaCachedError::Decode)?
        }
        _ => return Err(PostMetaCachedError::UnsupportedValue),
    };
    Ok(if matches!(result, PhpValue::Null) {
        PhpValue::Bytes(Vec::new())
    } else {
        result
    })
}

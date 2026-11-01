use recastpress_wp::{
    PostMetaCachedError as Error, PreparedPostMetaCache, get_post_meta_single_cached_no_filters,
    serialization::{LimitedUnserializeError as DecodeError, UnserializeLimits},
    value::{PhpArray, PhpKey, PhpValue},
};

fn bytes(raw: &[u8]) -> PhpValue {
    PhpValue::Bytes(raw.to_vec())
}

fn list(values: &[PhpValue]) -> PhpValue {
    let mut array = PhpArray::new();
    for (index, value) in values.iter().enumerate() {
        array.insert(
            PhpKey::Integer(i64::try_from(index).unwrap()),
            value.clone(),
        );
    }
    PhpValue::Array(array)
}

fn limits(input: usize, entries: usize, owned: usize, depth: usize) -> UnserializeLimits {
    UnserializeLimits::new(input, entries, owned, depth).unwrap()
}

fn generous() -> UnserializeLimits {
    limits(usize::MAX, usize::MAX, usize::MAX, 64)
}

fn get(cache: &PhpArray, key: &[u8]) -> Result<PhpValue, Error> {
    get_post_meta_single_cached_no_filters(&PreparedPostMetaCache::new(1, cache)?, key, generous())
}

#[test]
fn attachment_alt_single_raw_value_and_null_default_paths_preserve_false_and_empty_distinctions() {
    let mut cache = PhpArray::new();
    let alt = b"PRIVATE <literal> &copy; \0\xff";
    for (key, value) in [
        (b"_wp_attachment_image_alt".as_slice(), list(&[bytes(alt)])),
        (b"sql-null", list(&[PhpValue::Null])),
        (b"serialized-null", list(&[bytes(b"N;")])),
        (b"serialized-false", list(&[bytes(b"b:0;")])),
        (b"serialized-empty", list(&[bytes(b"s:0:\"\";")])),
        (b"raw-empty", list(&[bytes(b"")])),
        (b"isset-null", PhpValue::Null),
    ] {
        cache.insert(PhpKey::from_bytes(key), value);
    }
    let before = cache.clone();
    assert_eq!(get(&cache, b"_wp_attachment_image_alt"), Ok(bytes(alt)));
    assert_eq!(get(&cache, b"serialized-false"), Ok(PhpValue::Bool(false)));
    for key in [
        b"missing".as_slice(),
        b"sql-null",
        b"serialized-null",
        b"serialized-empty",
        b"raw-empty",
        b"isset-null",
    ] {
        assert_eq!(get(&cache, key), Ok(bytes(b"")));
    }
    assert_eq!(cache, before);
    // This is the raw get_post_meta return, not attachment markup or escaping.
    assert_eq!(get(&cache, b"_WP_attachment_image_alt"), Ok(bytes(b"")));
}

#[test]
fn positive_id_nonempty_complete_cache_and_truthy_key_are_explicit_profile_boundaries() {
    let mut cache = PhpArray::new();
    assert!(matches!(
        PreparedPostMetaCache::new(1, &cache),
        Err(Error::UnsupportedValue)
    ));
    cache.insert(PhpKey::Integer(0), list(&[bytes(b"zero")]));
    for id in [i64::MIN, -1, 0] {
        assert!(matches!(
            PreparedPostMetaCache::new(id, &cache),
            Err(Error::UnsupportedValue)
        ));
    }
    assert!(PreparedPostMetaCache::new(i64::MAX, &cache).is_ok());
    for key in [b"".as_slice(), b"0"] {
        assert_eq!(get(&cache, key), Err(Error::UnsupportedValue));
    }
    // The view does not inspect unrelated malformed values or infer DB identity.
    cache.insert(PhpKey::from_bytes(b"unrelated"), PhpValue::Unsupported);
    assert_eq!(get(&cache, b"absent"), Ok(bytes(b"")));
}

#[test]
fn borrowed_lookup_preserves_php_numeric_key_aliases_integer_extremes_and_binary_keys() {
    let mut cache = PhpArray::new();
    for (key, value) in [
        (i64::MIN, b"minimum".as_slice()),
        (i64::MAX, b"maximum"),
        (-1, b"minus"),
        (1, b"one"),
    ] {
        cache.insert(PhpKey::Integer(key), list(&[bytes(value)]));
    }
    for key in [
        b"01".as_slice(),
        b"+1",
        b"-0",
        b"9223372036854775808",
        b"-9223372036854775809",
        b"x\0\xff",
    ] {
        cache.insert(PhpKey::from_bytes(key), list(&[bytes(key)]));
    }
    let before = cache.clone();
    for (key, value) in [
        (b"-9223372036854775808".as_slice(), b"minimum".as_slice()),
        (b"9223372036854775807", b"maximum"),
        (b"-1", b"minus"),
        (b"1", b"one"),
        (b"01", b"01"),
        (b"+1", b"+1"),
        (b"-0", b"-0"),
        (b"9223372036854775808", b"9223372036854775808"),
        (b"-9223372036854775809", b"-9223372036854775809"),
        (b"x\0\xff", b"x\0\xff"),
    ] {
        assert_eq!(get(&cache, key), Ok(bytes(value)));
    }
    // Constructing with a numeric Bytes key applies the same normalization.
    cache.insert(PhpKey::Bytes(b"1".to_vec()), list(&[bytes(b"replacement")]));
    assert_eq!(get(&cache, b"1"), Ok(bytes(b"replacement")));
    assert_eq!(get(&before, b"1"), Ok(bytes(b"one")));
}

#[test]
fn selected_list_requires_dense_raw_values_but_only_first_bytes_are_decoded_and_charged() {
    let mut cache = PhpArray::new();
    cache.insert(
        PhpKey::from_bytes(b"selected"),
        list(&[bytes(b"ok"), bytes(b"O:1:\"X\":0:{}"), PhpValue::Null]),
    );
    cache.insert(PhpKey::from_bytes(b"unrelated"), PhpValue::Unsupported);
    let exact = limits(2, 0, 2, 0);
    assert_eq!(
        get_post_meta_single_cached_no_filters(
            &PreparedPostMetaCache::new(1, &cache).unwrap(),
            b"selected",
            exact
        ),
        Ok(bytes(b"ok"))
    );
    let mut sparse = PhpArray::new();
    sparse.insert(PhpKey::Integer(1), bytes(b"first"));
    let mut reversed = PhpArray::new();
    reversed.insert(PhpKey::Integer(1), bytes(b"one"));
    reversed.insert(PhpKey::Integer(0), bytes(b"zero"));
    for bad in [
        PhpValue::Bool(false),
        bytes(b"not-a-list"),
        PhpValue::Array(PhpArray::new()),
        PhpValue::Array(sparse),
        PhpValue::Array(reversed),
        list(&[PhpValue::Integer(1)]),
        list(&[bytes(b"ok"), PhpValue::Unsupported]),
        list(&[PhpValue::Array(PhpArray::new())]),
    ] {
        cache.insert(PhpKey::from_bytes(b"selected"), bad);
        assert_eq!(get(&cache, b"selected"), Err(Error::UnsupportedValue));
    }
    cache.insert(
        PhpKey::from_bytes(b"selected"),
        list(&[PhpValue::Null, bytes(b"O:1:\"X\":0:{}")]),
    );
    assert_eq!(
        get_post_meta_single_cached_no_filters(
            &PreparedPostMetaCache::new(1, &cache).unwrap(),
            b"selected",
            limits(0, 0, 0, 0)
        ),
        Ok(bytes(b""))
    );
}

#[test]
fn per_selected_value_four_limits_remain_typed_and_null_mapping_does_not_bypass_decode() {
    let raw = b"a:1:{s:1:\"k\";s:3:\"a\0b\";}";
    let mut cache = PhpArray::new();
    cache.insert(
        PhpKey::from_bytes(b"selected"),
        list(&[bytes(raw), bytes(&[b'x'; 4096])]),
    );
    let view = PreparedPostMetaCache::new(1, &cache).unwrap();
    let expected =
        get_post_meta_single_cached_no_filters(&view, b"selected", limits(raw.len(), 1, 4, 1))
            .unwrap();
    assert!(matches!(expected, PhpValue::Array(_)));
    for (limit, error) in [
        (limits(raw.len() - 1, 1, 4, 1), DecodeError::InputLimit),
        (limits(raw.len(), 0, 4, 1), DecodeError::EntryLimit),
        (limits(raw.len(), 1, 3, 1), DecodeError::OwnedBytesLimit),
        (limits(raw.len(), 1, 4, 0), DecodeError::DepthLimit),
    ] {
        assert_eq!(
            get_post_meta_single_cached_no_filters(&view, b"selected", limit),
            Err(Error::Decode(error))
        );
    }
    cache.insert(PhpKey::from_bytes(b"selected"), list(&[bytes(b"N;")]));
    assert_eq!(
        get_post_meta_single_cached_no_filters(
            &PreparedPostMetaCache::new(1, &cache).unwrap(),
            b"selected",
            limits(1, 0, 0, 0)
        ),
        Err(Error::Decode(DecodeError::InputLimit))
    );
    cache.insert(
        PhpKey::from_bytes(b"selected"),
        list(&[bytes(b"O:1:\"X\":0:{}")]),
    );
    assert_eq!(
        get(&cache, b"selected"),
        Err(Error::Decode(DecodeError::UnsupportedValue))
    );
}

#[test]
fn returned_values_are_owned_and_view_and_errors_never_debug_private_keys_or_values() {
    let result = {
        let mut cache = PhpArray::new();
        cache.insert(
            PhpKey::from_bytes(b"PRIVATE-key"),
            list(&[bytes(b"PRIVATE-value\0\xff")]),
        );
        let before = cache.clone();
        let view = PreparedPostMetaCache::new(123, &cache).unwrap();
        assert_eq!(format!("{view:?}"), "PreparedPostMetaCache([redacted])");
        let result =
            get_post_meta_single_cached_no_filters(&view, b"PRIVATE-key", generous()).unwrap();
        assert_eq!(cache, before);
        result
    };
    assert_eq!(result, bytes(b"PRIVATE-value\0\xff"));
    for error in [
        Error::UnsupportedValue,
        Error::Decode(DecodeError::UnsupportedValue),
        Error::Decode(DecodeError::InputLimit),
        Error::Decode(DecodeError::AllocationFailed),
    ] {
        let text = format!("{error:?} {error}");
        assert!(!text.contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

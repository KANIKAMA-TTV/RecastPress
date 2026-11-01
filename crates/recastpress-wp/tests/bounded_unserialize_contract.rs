use recastpress_wp::{
    CompatibilityError, maybe_unserialize,
    serialization::{
        LimitedUnserializeError as Error, UnserializeLimits, maybe_unserialize_bytes_with_limits,
    },
    value::{PhpKey, PhpValue},
};

fn limits(input: usize, pairs: usize, owned: usize, depth: usize) -> UnserializeLimits {
    UnserializeLimits::new(input, pairs, owned, depth).unwrap()
}

fn generous() -> UnserializeLimits {
    limits(usize::MAX, usize::MAX, usize::MAX, 64)
}

fn decode(raw: &[u8]) -> Result<PhpValue, Error> {
    maybe_unserialize_bytes_with_limits(raw, generous())
}

#[test]
fn input_and_unchanged_string_limits_include_outer_bytes_before_recognition() {
    let raw = b"\0 \rPRIVATE\xff\t";
    let before = raw.to_vec();
    assert_eq!(
        maybe_unserialize_bytes_with_limits(raw, limits(raw.len(), 0, raw.len(), 0)),
        Ok(PhpValue::Bytes(before.clone()))
    );
    assert_eq!(
        maybe_unserialize_bytes_with_limits(raw, limits(raw.len() - 1, 0, 0, 0)),
        Err(Error::InputLimit)
    );
    assert_eq!(
        maybe_unserialize_bytes_with_limits(raw, limits(raw.len(), 0, raw.len() - 1, 0)),
        Err(Error::OwnedBytesLimit)
    );
    assert_eq!(raw.as_slice(), before);
    assert_eq!(
        maybe_unserialize_bytes_with_limits(b"", limits(0, 0, 0, 0)),
        Ok(PhpValue::Bytes(Vec::new()))
    );
    for (raw, value) in [
        (b"\0 \tN;\r\n\x0b".as_slice(), PhpValue::Null),
        (b"b:0;", PhpValue::Bool(false)),
        (b"b:1;", PhpValue::Bool(true)),
        (b"i:-9223372036854775808;", PhpValue::Integer(i64::MIN)),
        (b"i:9223372036854775807;", PhpValue::Integer(i64::MAX)),
        (b"s:0:\"\";", PhpValue::Bytes(Vec::new())),
    ] {
        assert_eq!(
            maybe_unserialize_bytes_with_limits(raw, limits(raw.len(), 0, 0, 0)),
            Ok(value)
        );
    }
    assert_eq!(
        maybe_unserialize_bytes_with_limits(b" N; ", limits(3, 0, 0, 0)),
        Err(Error::InputLimit)
    );
}

#[test]
fn cumulative_pairs_key_and_payload_bytes_and_array_depth_have_independent_exact_bounds() {
    let raw = b"a:2:{s:1:\"k\";s:3:\"a\0b\";i:0;a:1:{s:1:\"x\";s:0:\"\";}}";
    let before = raw.to_vec();
    let exact = maybe_unserialize_bytes_with_limits(raw, limits(raw.len(), 3, 5, 2)).unwrap();
    assert_eq!(exact, decode(raw).unwrap());
    for (bound, error) in [
        (limits(raw.len() - 1, 3, 5, 2), Error::InputLimit),
        (limits(raw.len(), 2, 5, 2), Error::EntryLimit),
        (limits(raw.len(), 3, 4, 2), Error::OwnedBytesLimit),
        (limits(raw.len(), 3, 5, 1), Error::DepthLimit),
    ] {
        assert_eq!(maybe_unserialize_bytes_with_limits(raw, bound), Err(error));
    }
    assert_eq!(raw.as_slice(), before);
    assert_eq!(
        maybe_unserialize_bytes_with_limits(b"a:0:{}", limits(6, 0, 0, 1)),
        Ok(PhpValue::Array(Default::default()))
    );
    assert_eq!(
        maybe_unserialize_bytes_with_limits(b"a:0:{}", limits(6, 0, 0, 0)),
        Err(Error::DepthLimit)
    );
    assert_eq!(
        maybe_unserialize_bytes_with_limits(b"s:3:\"\xff\r\0\";", limits(10, 0, 3, 0)),
        Ok(PhpValue::Bytes(b"\xff\r\0".to_vec()))
    );
}

#[test]
fn canonical_key_types_order_binary_strings_and_one_layer_are_preserved() {
    let raw = b"a:6:{i:0;N;s:2:\"01\";b:0;s:2:\"-0\";s:2:\"N;\";s:2:\"+1\";i:2;s:2:\"\0\xff\";s:3:\"a\0b\";s:19:\"9223372036854775808\";i:3;}";
    assert_eq!(
        decode(raw).unwrap(),
        maybe_unserialize(&PhpValue::Bytes(raw.to_vec())).unwrap()
    );
    let PhpValue::Array(array) = decode(raw).unwrap() else {
        panic!("array expected");
    };
    assert_eq!(array.iter().next().unwrap().0, &PhpKey::Integer(0));
    assert_eq!(
        array.get(&PhpKey::from_bytes(b"-0")),
        Some(&PhpValue::Bytes(b"N;".to_vec()))
    );
    assert_eq!(decode(b"s:2:\"N;\";"), Ok(PhpValue::Bytes(b"N;".to_vec())));
    for key in [b"-9223372036854775808".as_slice(), b"9223372036854775807"] {
        let raw = format!(
            "a:1:{{s:{}:\"{}\";N;}}",
            key.len(),
            std::str::from_utf8(key).unwrap()
        );
        assert_eq!(decode(raw.as_bytes()), Err(Error::UnsupportedValue));
        assert_eq!(
            maybe_unserialize(&PhpValue::Bytes(raw.into_bytes())),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
    for key in [
        b"-9223372036854775809".as_slice(),
        b"9223372036854775808",
        b"1e0",
    ] {
        let raw = format!(
            "a:1:{{s:{}:\"{}\";N;}}",
            key.len(),
            std::str::from_utf8(key).unwrap()
        )
        .into_bytes();
        let bounded = decode(&raw).unwrap();
        assert_eq!(bounded, maybe_unserialize(&PhpValue::Bytes(raw)).unwrap());
        let PhpValue::Array(array) = bounded else {
            panic!("array expected");
        };
        assert_eq!(array.iter().next().unwrap().0, &PhpKey::Bytes(key.to_vec()));
    }
    for invalid in [
        b"i:+1;".as_slice(),
        b"i:-0;",
        b"i:01;",
        b"i:9223372036854775808;",
        b"s:01:\"x\";",
        b"a:00:{}",
        b"b:222;",
        b"d:1.5;",
        b"O:1:\"X\":0:{}",
        b"a:1:{s:1:\"0\";N;}",
        b"a:1:{s:2:\"-1\";N;}",
        b"a:2:{i:0;N;i:0;b:1;}",
        b"a:2:{s:1:\"x\";N;s:1:\"x\";b:1;}",
        b"a:1:{i:0;R:1;}",
        b"s:1:\"x\";s:1:\"y\";",
    ] {
        assert_eq!(decode(invalid), Err(Error::UnsupportedValue));
    }
    // The shallow WP recognizer is not replaced with a broad format rejector.
    for unchanged in [
        b"R:1;".as_slice(),
        b"i:1;tail",
        b"\x0cN;\x0c",
        b"s:not-a-length\";",
    ] {
        assert_eq!(decode(unchanged), Ok(PhpValue::Bytes(unchanged.to_vec())));
    }
}

#[test]
fn declared_counts_do_not_drive_allocation_and_depth_64_matches_legacy_ceiling() {
    for raw in [
        b"a:999999999999999999999999999999:{}".as_slice(),
        b"a:999999999:{}",
    ] {
        assert_eq!(decode(raw), Err(Error::UnsupportedValue));
    }
    let mut nested = b"a:1:{i:0;".repeat(64);
    nested.extend_from_slice(b"N;");
    nested.extend_from_slice(&[b'}'; 64]);
    assert_eq!(
        maybe_unserialize_bytes_with_limits(&nested, limits(nested.len(), 64, 0, 64)).unwrap(),
        maybe_unserialize(&PhpValue::Bytes(nested.clone())).unwrap()
    );
    assert_eq!(
        maybe_unserialize_bytes_with_limits(&nested, limits(nested.len(), 63, 0, 64)),
        Err(Error::EntryLimit)
    );
    let mut too_deep = b"a:1:{i:0;".to_vec();
    too_deep.extend_from_slice(&nested);
    too_deep.push(b'}');
    assert_eq!(decode(&too_deep), Err(Error::DepthLimit));
    assert_eq!(
        UnserializeLimits::new(usize::MAX, usize::MAX, usize::MAX, 65),
        Err(Error::InvalidLimits)
    );
}

#[test]
fn generous_limits_match_legacy_for_finite_byte_mutations_without_changing_inputs() {
    for original in [
        b"N;".as_slice(),
        b"b:0;",
        b"i:-10;",
        b"s:3:\"a\0b\";",
        b"a:1:{i:0;N;}",
        b"a:1:{s:1:\"x\";b:1;}",
        b"a:0:{}",
        b"\0\xffraw\r",
    ] {
        for position in 0..=original.len() {
            for byte in 0..=255 {
                let mut raw = original.to_vec();
                if position == raw.len() {
                    raw.push(byte);
                } else {
                    raw[position] = byte;
                }
                let before = raw.clone();
                let old = maybe_unserialize(&PhpValue::Bytes(raw.clone()));
                let bounded = decode(&raw);
                match old {
                    Ok(value) => assert_eq!(bounded, Ok(value)),
                    Err(CompatibilityError::RecursionLimit) => {
                        assert_eq!(bounded, Err(Error::DepthLimit))
                    }
                    Err(_) => assert_eq!(bounded, Err(Error::UnsupportedValue)),
                }
                assert_eq!(raw, before);
            }
        }
    }
}

#[test]
fn limits_and_errors_are_closed_and_failed_values_are_never_returned_as_false_or_null() {
    let secret = b"a:1:{s:7:\"PRIVATE\";O:1:\"X\":0:{}}";
    assert_eq!(decode(secret), Err(Error::UnsupportedValue));
    let errors = [
        Error::InvalidLimits,
        Error::UnsupportedValue,
        Error::InputLimit,
        Error::EntryLimit,
        Error::OwnedBytesLimit,
        Error::DepthLimit,
        Error::AllocationFailed,
    ];
    for error in errors {
        let text = format!("{error:?} {error}");
        assert!(!text.contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
    assert_eq!(decode(b"N;"), Ok(PhpValue::Null));
    assert_eq!(decode(b"b:0;"), Ok(PhpValue::Bool(false)));
    assert_eq!(decode(b"s:0:\"\";"), Ok(PhpValue::Bytes(Vec::new())));
}

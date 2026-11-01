use recastpress_wp::maybe_unserialize;
use recastpress_wp::{CompatibilityError, is_serialized, is_serialized_string, value::PhpValue};
use recastpress_wp::{
    maybe_serialize,
    value::{PhpArray, PhpKey},
};

#[test]
fn maybe_unserialize_canonical_arrays_roundtrip_keys_types_and_binary_lengths() {
    let encoded =
        bytes(b"a:3:{s:2:\"01\";i:-9223372036854775808;i:2;s:3:\"a\0b\";s:2:\"\0\xff\";b:0;}");
    let decoded = maybe_unserialize(&encoded).unwrap();
    let PhpValue::Array(array) = &decoded else {
        panic!("serialized array did not decode")
    };
    assert_eq!(
        array.get(&PhpKey::from_bytes(b"01")),
        Some(&PhpValue::Integer(i64::MIN))
    );
    assert_eq!(array.get(&PhpKey::Integer(2)), Some(&bytes(b"a\0b")));
    assert_eq!(
        array.get(&PhpKey::from_bytes(b"\0\xff")),
        Some(&PhpValue::Bool(false))
    );
    assert_eq!(maybe_serialize(&decoded).unwrap(), encoded);
}

#[test]
fn maybe_unserialize_trims_only_the_outer_frame_and_decodes_one_layer() {
    assert_eq!(
        maybe_unserialize(&bytes(b"\0 \tN;\r\n\x0b")).unwrap(),
        PhpValue::Null
    );
    assert_eq!(
        maybe_unserialize(&bytes(b"s:2:\"N;\";")).unwrap(),
        bytes(b"N;")
    );
    for value in [
        bytes(b"plain"),
        bytes(b"\x0cN;\x0c"),
        bytes(b"R:1;"),
        PhpValue::Integer(1),
        PhpValue::Float(1.5),
    ] {
        assert_eq!(maybe_unserialize(&value).unwrap(), value);
    }
}

#[test]
fn maybe_unserialize_rejects_noncanonical_malformed_and_active_encodings() {
    for value in [
        b"b:222;".as_slice(),
        b"i:+1;",
        b"i:9223372036854775808;",
        b"s:99:\"x\";",
        b"s:1:\"x\";s:1:\"y\";",
        b"a:999999999999:{}",
        b"a:1:{i:0;R:1;}",
        b"a:1:{s:1:\"1\";i:2;}",
        b"O:1:\"X\":0:{}",
        b"d:1.5;",
    ] {
        assert_eq!(
            maybe_unserialize(&bytes(value)),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
    assert_eq!(
        maybe_unserialize(&PhpValue::Unsupported),
        Err(CompatibilityError::UnsupportedValue)
    );
}

#[test]
fn maybe_unserialize_bounds_array_depth_before_allocating_from_counts() {
    let mut encoded = b"a:1:{i:0;".repeat(64);
    encoded.extend_from_slice(b"N;");
    encoded.extend_from_slice(&[b'}'; 64]);
    assert!(maybe_unserialize(&bytes(&encoded)).is_ok());
    let mut excessive = b"a:1:{i:0;".to_vec();
    excessive.extend_from_slice(&encoded);
    excessive.push(b'}');
    assert_eq!(
        maybe_unserialize(&bytes(&excessive)),
        Err(CompatibilityError::RecursionLimit)
    );
}

#[test]
fn maybe_unserialize_byte_mutations_do_not_panic_or_trust_declared_lengths() {
    for original in [b"N;".as_slice(), b"s:3:\"a\0b\";", b"a:1:{i:0;N;}"] {
        for index in 0..=original.len() {
            for byte in 0..=255 {
                let mut mutated = original.to_vec();
                if index == mutated.len() {
                    mutated.push(byte);
                } else {
                    mutated[index] = byte;
                }
                assert!(std::panic::catch_unwind(|| maybe_unserialize(&bytes(&mutated))).is_ok());
            }
        }
    }
}

#[test]
fn maybe_serialize_preserves_original_bytes_when_double_encoding_shallow_matches() {
    assert_eq!(
        maybe_serialize(&bytes(b" \0N;\t")).unwrap(),
        bytes(b"s:5:\" \0N;\t\";")
    );
    assert_eq!(
        maybe_serialize(&bytes(b"b:222;")).unwrap(),
        bytes(b"s:6:\"b:222;\";")
    );
    assert_eq!(
        maybe_serialize(&bytes(b"ordinary\0\xff")).unwrap(),
        bytes(b"ordinary\0\xff")
    );
    assert_eq!(
        maybe_serialize(&PhpValue::Float(-0.0)).unwrap(),
        PhpValue::Float(-0.0)
    );
}

#[test]
fn maybe_serialize_array_keeps_key_order_raw_string_lengths_and_scalar_types() {
    let mut array = PhpArray::new();
    array.insert(PhpKey::from_bytes(b"1"), bytes("🦀".as_bytes()));
    array.insert(PhpKey::from_bytes(b"01"), PhpValue::Null);
    array.insert(PhpKey::Integer(-2), PhpValue::Bool(true));
    array.insert(PhpKey::from_bytes(b"\0\xff"), PhpValue::Integer(i64::MIN));
    assert_eq!(maybe_serialize(&PhpValue::Array(array)).unwrap(),bytes(b"a:4:{i:1;s:4:\"\xf0\x9f\xa6\x80\";s:2:\"01\";N;i:-2;b:1;s:2:\"\0\xff\";i:-9223372036854775808;}"));
}

#[test]
fn maybe_serialize_rejects_unmodeled_nested_values_and_host_depth_excess() {
    let mut nested = PhpValue::Null;
    for _ in 0..64 {
        let mut array = PhpArray::new();
        array.insert(PhpKey::Integer(0), nested);
        nested = PhpValue::Array(array);
    }
    assert!(maybe_serialize(&nested).is_ok());
    let mut deeper = PhpArray::new();
    deeper.insert(PhpKey::Integer(0), nested);
    assert_eq!(
        maybe_serialize(&PhpValue::Array(deeper)),
        Err(CompatibilityError::RecursionLimit)
    );
    for value in [PhpValue::Float(1.5), PhpValue::Unsupported] {
        let mut array = PhpArray::new();
        array.insert(PhpKey::Integer(0), value);
        assert_eq!(
            maybe_serialize(&PhpValue::Array(array)),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
    assert_eq!(
        maybe_serialize(&PhpValue::Unsupported),
        Err(CompatibilityError::UnsupportedValue)
    );
}
fn bytes(value: &[u8]) -> PhpValue {
    PhpValue::Bytes(value.to_vec())
}

#[test]
fn serialized_recognizers_preserve_wordpress_shallow_false_positives() {
    for data in [
        b"b:222;".as_slice(),
        b"i:+-E..;",
        b"a:999:garbage}",
        b"s:999:\"x\";",
        b"E:1:garbage}",
    ] {
        assert!(is_serialized(&bytes(data), true).unwrap());
    }
    assert!(is_serialized_string(&bytes(b"s:not-a-length\";")).unwrap());
    assert!(!is_serialized(&bytes(b"s:not-a-length\";"), true).unwrap());
    assert!(!is_serialized(&bytes(b"d:1e2;"), true).unwrap());
    assert!(is_serialized(&bytes(b"d:1E2;"), true).unwrap());
}

#[test]
fn serialized_strict_loose_and_php_trim_boundaries_are_distinct() {
    assert!(is_serialized(&bytes(b"\0\x0b N;\t\r\n"), true).unwrap());
    assert!(!is_serialized(&bytes(b"\x0cN;\x0c"), true).unwrap());
    assert!(!is_serialized(&bytes(b"i:1;tail"), true).unwrap());
    assert!(is_serialized(&bytes(b"i:1;tail"), false).unwrap());
    assert!(!is_serialized(&bytes(b"a:1}:garbage;"), false).unwrap());
    assert!(!is_serialized(&bytes(b"s:1:garbage;"), false).unwrap());
}

#[test]
fn serialized_nonstring_values_are_false_and_unknown_values_stay_explicit() {
    for value in [
        PhpValue::Null,
        PhpValue::Integer(1),
        PhpValue::Float(f64::NAN),
    ] {
        assert!(!is_serialized(&value, true).unwrap());
        assert!(!is_serialized_string(&value).unwrap());
    }
    assert_eq!(
        is_serialized(&PhpValue::Unsupported, true),
        Err(CompatibilityError::UnsupportedValue)
    );
    assert_eq!(
        is_serialized_string(&PhpValue::Unsupported),
        Err(CompatibilityError::UnsupportedValue)
    );
}

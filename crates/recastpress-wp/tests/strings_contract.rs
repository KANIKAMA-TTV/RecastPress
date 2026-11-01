use recastpress_wp::{
    CompatibilityError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_slash, wp_unslash,
};

#[test]
fn slash_transforms_bytes_values_and_retains_array_keys() {
    let mut original = PhpArray::new();
    original.insert(
        PhpKey::Bytes(b"key'\\\0".to_vec()),
        PhpValue::Bytes(b"'\"\\\0\xff".to_vec()),
    );
    original.insert(PhpKey::Integer(-7), PhpValue::Bool(false));
    let original = PhpValue::Array(original);
    let slashed = wp_slash(&original).unwrap();
    let PhpValue::Array(array) = &slashed else {
        panic!("expected array");
    };
    assert_eq!(
        array.iter().next().unwrap().0,
        &PhpKey::Bytes(b"key'\\\0".to_vec())
    );
    assert_eq!(
        array.iter().next().unwrap().1,
        &PhpValue::Bytes(b"\\'\\\"\\\\\\0\xff".to_vec())
    );
    assert_eq!(wp_unslash(&slashed), Ok(original));
    assert_eq!(
        wp_unslash(&PhpValue::Bytes(b"\\q\\n\\0trailing\\".to_vec())),
        Ok(PhpValue::Bytes(b"qn\0trailing".to_vec()))
    );
}

#[test]
fn recursive_string_transforms_reject_unsupported_values_and_excess_depth() {
    for transform in [wp_slash, wp_unslash] {
        assert_eq!(
            transform(&PhpValue::Unsupported),
            Err(CompatibilityError::UnsupportedValue)
        );
        let mut value = PhpValue::Bytes(b"quote'".to_vec());
        for _ in 0..64 {
            let mut array = PhpArray::new();
            array.insert(PhpKey::Integer(0), value);
            value = PhpValue::Array(array);
        }
        assert!(transform(&value).is_ok());
        let mut array = PhpArray::new();
        array.insert(PhpKey::Integer(0), value);
        assert_eq!(
            transform(&PhpValue::Array(array)),
            Err(CompatibilityError::RecursionLimit)
        );
    }
}

use recastpress_wp::{
    CompatibilityError,
    value::{PhpArray, PhpValue},
    wp_is_uuid,
};

fn bytes(value: &[u8]) -> PhpValue {
    PhpValue::Bytes(value.to_vec())
}

#[test]
fn uuid_default_is_a_lowercase_shape_check_and_v4_adds_version_and_variant() {
    let valid = bytes(b"123e4567-e89b-42d3-a456-426614174000");
    assert_eq!(wp_is_uuid(&valid, None), Ok(true));
    assert_eq!(wp_is_uuid(&valid, Some(4)), Ok(true));
    let zeros = bytes(b"00000000-0000-0000-0000-000000000000");
    assert_eq!(wp_is_uuid(&zeros, None), Ok(true));
    assert_eq!(wp_is_uuid(&zeros, Some(4)), Ok(false));
    assert_eq!(
        wp_is_uuid(&bytes(b"123E4567-e89b-42d3-a456-426614174000"), None),
        Ok(false)
    );
    assert_eq!(
        wp_is_uuid(&bytes(b"123e4567-e89b-42d3-c456-426614174000"), Some(4)),
        Ok(false)
    );
}

#[test]
fn uuid_keeps_the_source_final_newline_anchor_behavior() {
    let base = b"123e4567-e89b-42d3-a456-426614174000";
    for (suffix, valid) in [
        (b"\n".as_slice(), true),
        (b"\r\n", false),
        (b"\n\n", false),
        (b" ", false),
        (b"\0", false),
    ] {
        let input = [base.as_slice(), suffix].concat();
        assert_eq!(wp_is_uuid(&bytes(&input), Some(4)), Ok(valid));
    }
}

#[test]
fn uuid_nonstring_short_circuit_does_not_enter_unsupported_version_diagnostics() {
    for value in [
        PhpValue::Null,
        PhpValue::Bool(true),
        PhpValue::Integer(4),
        PhpValue::Float(f64::NAN),
        PhpValue::Array(PhpArray::new()),
    ] {
        assert_eq!(wp_is_uuid(&value, Some(5)), Ok(false));
    }
    assert_eq!(
        wp_is_uuid(&bytes(b"not a UUID"), Some(5)),
        Err(CompatibilityError::UnsupportedValue)
    );
}

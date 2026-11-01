use recastpress_wp::{
    value::{PhpArray, PhpValue},
    wp_check_jsonp_callback,
};

#[test]
fn jsonp_callback_preserves_the_permissive_byte_pattern_not_js_identifier_rules() {
    for bytes in [
        b"".as_slice(),
        b".",
        b"...",
        b"1",
        b"1callback",
        b"_a.B09",
        b"a..b",
        b".callback.",
    ] {
        assert!(wp_check_jsonp_callback(&PhpValue::Bytes(bytes.to_vec())));
    }
}

#[test]
fn jsonp_callback_rejects_every_other_byte_including_control_and_nonascii_bytes() {
    for byte in 0u8..=255 {
        let expected = byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.');
        for bytes in [
            vec![byte],
            vec![b'a', byte],
            vec![byte, b'b'],
            vec![b'a', byte, b'b'],
        ] {
            assert_eq!(wp_check_jsonp_callback(&PhpValue::Bytes(bytes)), expected);
        }
    }
}

#[test]
fn jsonp_callback_nonstring_values_return_false_without_casting() {
    for value in [
        PhpValue::Null,
        PhpValue::Bool(true),
        PhpValue::Integer(123),
        PhpValue::Float(f64::NAN),
        PhpValue::Float(f64::INFINITY),
        PhpValue::Array(PhpArray::new()),
    ] {
        assert!(!wp_check_jsonp_callback(&value));
    }
}

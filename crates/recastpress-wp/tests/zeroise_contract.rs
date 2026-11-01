use recastpress_wp::{
    ZeroiseError,
    value::{PhpArray, PhpValue},
    zeroise,
};

#[test]
fn signed_decimals_pad_before_sign_and_preserve_integer_extremes() {
    for (number, width, expected) in [
        (0, 0, "0"),
        (0, 3, "000"),
        (-1, 3, "0-1"),
        (-123, 6, "00-123"),
        (12345, 2, "12345"),
        (i64::MIN, 0, "-9223372036854775808"),
        (i64::MIN, 21, "0-9223372036854775808"),
        (i64::MAX, 20, "09223372036854775807"),
    ] {
        assert_eq!(
            zeroise(&PhpValue::Integer(number), width, expected.len()),
            Ok(expected.as_bytes().to_vec())
        );
    }
}

#[test]
fn raw_bytes_use_byte_length_without_parsing_truncation_or_input_changes() {
    for (input, width, expected) in [
        (b"".as_slice(), 0, b"".as_slice()),
        (b"", 3, b"000"),
        (b"00012", 2, b"00012"),
        (b"-001", 6, b"00-001"),
        (b"a\0b", 5, b"00a\0b"),
        (b"\xff\xc0\x80", 5, b"00\xff\xc0\x80"),
        ("日本語".as_bytes(), 5, "日本語".as_bytes()),
    ] {
        let value = PhpValue::Bytes(input.to_vec());
        let original = value.clone();
        assert_eq!(
            zeroise(&value, width, expected.len()),
            Ok(expected.to_vec())
        );
        assert_eq!(value, original);
        if !expected.is_empty() {
            assert_eq!(
                zeroise(&value, width, expected.len() - 1),
                Err(ZeroiseError::OutputLimit)
            );
        }
    }
}

#[test]
fn width_bound_and_complete_output_cap_are_separate_host_limits() {
    let value = PhpValue::Integer(-1);
    let output = zeroise(&value, 4096, 4096).unwrap();
    assert_eq!(output.len(), 4096);
    assert_eq!(&output[4094..], b"-1");
    assert!(output[..4094].iter().all(|byte| *byte == b'0'));
    assert_eq!(zeroise(&value, 4096, 4095), Err(ZeroiseError::OutputLimit));
    for width in [4097, usize::MAX] {
        assert_eq!(zeroise(&value, width, 0), Err(ZeroiseError::WidthLimit));
    }
    let long_input = PhpValue::Bytes(vec![b'x'; 5000]);
    assert_eq!(zeroise(&long_input, 0, 5000), Ok(vec![b'x'; 5000]));
    assert_eq!(zeroise(&value, 0, 1), Err(ZeroiseError::OutputLimit));
}

#[test]
fn unsupported_types_are_rejected_before_host_width_or_cap_checks() {
    for value in [
        PhpValue::Null,
        PhpValue::Bool(false),
        PhpValue::Bool(true),
        PhpValue::Float(1.25),
        PhpValue::Array(PhpArray::new()),
        PhpValue::Unsupported,
    ] {
        assert_eq!(
            zeroise(&value, usize::MAX, 0),
            Err(ZeroiseError::UnsupportedValue)
        );
    }
}

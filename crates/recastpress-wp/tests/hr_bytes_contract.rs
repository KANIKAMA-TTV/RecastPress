use recastpress_wp::{CompatibilityError, wp_convert_hr_to_bytes_default_units_bytes as convert};

#[test]
fn decimal_digits_leading_zeros_and_default_unit_cases_are_exact() {
    for (input, expected) in [
        (b"0".as_slice(), 0),
        (b"000000G", 0),
        (b"000000123", 123),
        (b"1k", 1024),
        (b"002K", 2048),
        (b"3m", 3 * 1024 * 1024),
        (b"4M", 4 * 1024 * 1024),
        (b"5g", 5 * 1024 * 1024 * 1024),
        (b"6G", 6 * 1024 * 1024 * 1024),
    ] {
        let original = input.to_vec();
        assert_eq!(convert(input), Ok(expected));
        assert_eq!(input, original);
    }
}

#[test]
fn positive_integer_and_float_product_boundaries_saturate_to_integer_max() {
    assert_eq!(convert(b"9223372036854775806"), Ok(i64::MAX - 1));
    for input in [
        b"9223372036854775807".as_slice(),
        b"9223372036854775808",
        b"18446744073709551615",
    ] {
        assert_eq!(convert(input), Ok(i64::MAX));
    }
    for (suffix, multiplier, first_overflow) in [
        (b'K', 1024i64, 9007199254740992u64),
        (b'M', 1024 * 1024, 8796093022208),
        (b'G', 1024 * 1024 * 1024, 8589934592),
    ] {
        for delta in [-1i64, 0, 1] {
            let number = first_overflow.checked_add_signed(delta).unwrap();
            let input = format!("{number}{}", char::from(suffix));
            let expected = if delta < 0 {
                (i64::MAX / multiplier) * multiplier
            } else {
                i64::MAX
            };
            assert_eq!(convert(input.as_bytes()), Ok(expected));
        }
    }
    assert_eq!(convert(&vec![b'9'; 256]), Ok(i64::MAX));
    assert_eq!(
        convert(&vec![b'9'; 257]),
        Err(CompatibilityError::UnsupportedValue)
    );
    let mut zeros = vec![b'0'; 16384];
    zeros.extend_from_slice(b"1G");
    assert_eq!(convert(&zeros), Ok(1024 * 1024 * 1024));
    let mut leading = vec![b'0'; 4096];
    leading.extend_from_slice(&[b'9'; 256]);
    leading.push(b'K');
    assert_eq!(convert(&leading), Ok(i64::MAX));
    leading.insert(4096, b'9');
    assert_eq!(convert(&leading), Err(CompatibilityError::UnsupportedValue));
    assert_eq!(convert(&vec![b'0'; 16384]), Ok(0));
}

#[test]
fn complete_grammar_validation_rejects_cast_trim_and_contains_domains_before_saturation() {
    for input in [
        b"".as_slice(),
        b"k",
        b"+1",
        b"-1",
        b" 1K",
        b"1K ",
        b"\x0b1",
        b"1.5G",
        b"1e3G",
        b"1KB",
        b"1kmg",
        b"1Ggarbage",
        b"1\0G",
        b"1\xffG",
        "１G".as_bytes(),
    ] {
        assert_eq!(
            convert(input),
            Err(CompatibilityError::UnsupportedValue),
            "{input:?}"
        );
    }
    let mut huge = vec![b'9'; 16384];
    huge.extend_from_slice(b"xG");
    assert_eq!(convert(&huge), Err(CompatibilityError::UnsupportedValue));
    let mut leading = vec![b'0'; 16384];
    leading.extend_from_slice(b"1xG");
    assert_eq!(convert(&leading), Err(CompatibilityError::UnsupportedValue));
    for byte in 0u8..=255 {
        let input = [b'1', byte, b'G'];
        if byte.is_ascii_digit() {
            assert!(convert(&input).is_ok());
        } else {
            assert_eq!(convert(&input), Err(CompatibilityError::UnsupportedValue));
        }
    }
}

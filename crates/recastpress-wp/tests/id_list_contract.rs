use recastpress_wp::{
    CompatibilityError, absint_integer,
    value::{PhpArray, PhpKey, PhpValue},
    wp_parse_id_list_integers,
};
use recastpress_wp::{absint, wp_parse_id_list};

#[test]
fn representable_float_casts_truncate_but_do_not_saturate_overflow_or_hide_diagnostics() {
    for (value, expected) in [
        (0.0, 0),
        (-0.0, 0),
        (1.9, 1),
        (-1.9, 1),
        (f64::from_bits(1), 0),
        (9_223_372_036_854_774_784.0, 9_223_372_036_854_774_784),
    ] {
        assert_eq!(
            absint(&PhpValue::Float(value)),
            Ok(PhpValue::Integer(expected))
        );
    }
    assert_eq!(
        absint(&PhpValue::Float(-9_223_372_036_854_775_808.0)),
        Ok(absint_integer(i64::MIN))
    );
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        9_223_372_036_854_775_808.0,
        f64::from_bits((-9_223_372_036_854_775_808.0_f64).to_bits() + 1),
    ] {
        assert_eq!(
            absint(&PhpValue::Float(value)),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
}

#[test]
fn float_id_values_deduplicate_after_truncation_and_keep_the_first_key() {
    let mut input = PhpArray::new();
    for (key, value) in [
        (b"first".as_slice(), PhpValue::Float(-1.9)),
        (b"again", PhpValue::Integer(1)),
        (b"zero", PhpValue::Float(-0.0)),
        (b"minimum", PhpValue::Float(-9_223_372_036_854_775_808.0)),
        (b"maximum", PhpValue::Integer(i64::MAX)),
    ] {
        input.insert(PhpKey::from_bytes(key), value);
    }
    let parsed = wp_parse_id_list(&PhpValue::Array(input)).unwrap();
    assert_eq!(parsed.len(), 4);
    assert_eq!(
        parsed.get(&PhpKey::from_bytes(b"first")),
        Some(&PhpValue::Integer(1))
    );
    assert_eq!(parsed.get(&PhpKey::from_bytes(b"again")), None);
    assert_eq!(
        parsed.get(&PhpKey::from_bytes(b"minimum")),
        Some(&absint_integer(i64::MIN))
    );
    assert_eq!(
        parsed.get(&PhpKey::from_bytes(b"maximum")),
        Some(&PhpValue::Integer(i64::MAX))
    );
}

#[test]
fn absint_supported_coercions_do_not_guess_fraction_exponent_or_overflow_strings() {
    for (value, expected) in [
        (PhpValue::Null, 0),
        (PhpValue::Bool(false), 0),
        (PhpValue::Bool(true), 1),
        (PhpValue::Bytes(b"\x0b\x0c +0012 \t\r\n".to_vec()), 12),
        (PhpValue::Bytes(vec![]), 0),
        (PhpValue::Array(PhpArray::new()), 0),
    ] {
        assert_eq!(absint(&value).unwrap(), PhpValue::Integer(expected));
    }
    for value in [
        b"1.2".as_slice(),
        b"1e2",
        b"9223372036854775808",
        b"-9223372036854775809",
        b"12tail",
        b"\0",
        b"\xff",
        b"+",
        b"1_2",
    ] {
        assert_eq!(
            absint(&PhpValue::Bytes(value.to_vec())),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
    assert_eq!(
        absint(&PhpValue::Bytes(b"-9223372036854775808".to_vec())).unwrap(),
        absint_integer(i64::MIN)
    );
}

#[test]
fn string_id_list_keeps_duplicate_holes_and_integer_boundaries() {
    let input =
        PhpValue::Bytes(b" 1,-1,+002\t0,-0 -9223372036854775808 9223372036854775807 ".to_vec());
    let result = wp_parse_id_list(&input).unwrap();
    assert_eq!(
        result
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        [
            PhpKey::Integer(0),
            PhpKey::Integer(2),
            PhpKey::Integer(3),
            PhpKey::Integer(5),
            PhpKey::Integer(6)
        ]
    );
    assert_eq!(
        result.get(&PhpKey::Integer(5)),
        Some(&absint_integer(i64::MIN))
    );
    assert_eq!(
        result.get(&PhpKey::Integer(6)),
        Some(&PhpValue::Integer(i64::MAX))
    );
    assert_eq!(
        wp_parse_id_list(&PhpValue::Bytes(b"1,2,1.5".to_vec())),
        Err(CompatibilityError::UnsupportedValue)
    );
}

#[test]
fn absint_keeps_minimum_integer_as_float_without_wrapping_or_clamping() {
    assert_eq!(absint_integer(-1), PhpValue::Integer(1));
    assert_eq!(absint_integer(0), PhpValue::Integer(0));
    assert_eq!(absint_integer(i64::MAX), PhpValue::Integer(i64::MAX));
    let PhpValue::Float(minimum) = absint_integer(i64::MIN) else {
        panic!("minimum integer did not become a PHP float")
    };
    assert_eq!(minimum.to_bits(), 0x43e0_0000_0000_0000);
}

#[test]
fn id_list_preserves_first_keys_holes_and_distinct_boundary_magnitudes() {
    let mut input = PhpArray::new();
    for (key, value) in [
        (PhpKey::Integer(-9), PhpValue::Integer(-1)),
        (PhpKey::Integer(20), PhpValue::Integer(1)),
        (PhpKey::from_bytes(b"zero"), PhpValue::Integer(0)),
        (PhpKey::Integer(100), PhpValue::Integer(-2)),
        (PhpKey::Integer(101), PhpValue::Integer(2)),
        (PhpKey::from_bytes(b"minimum"), PhpValue::Integer(i64::MIN)),
        (PhpKey::from_bytes(b"maximum"), PhpValue::Integer(i64::MAX)),
        (
            PhpKey::from_bytes(b"minimum_again"),
            PhpValue::Integer(i64::MIN),
        ),
        (PhpKey::from_bytes(b"null"), PhpValue::Null),
        (
            PhpKey::from_bytes(b"array"),
            PhpValue::Array(PhpArray::new()),
        ),
    ] {
        input.insert(key, value);
    }
    let result = wp_parse_id_list_integers(&input).unwrap();
    assert_eq!(wp_parse_id_list(&PhpValue::Array(input)).unwrap(), result);
    assert_eq!(
        result
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        [
            PhpKey::Integer(-9),
            PhpKey::from_bytes(b"zero"),
            PhpKey::Integer(100),
            PhpKey::from_bytes(b"minimum"),
            PhpKey::from_bytes(b"maximum"),
        ]
    );
    assert_eq!(
        result.get(&PhpKey::Integer(-9)),
        Some(&PhpValue::Integer(1))
    );
    assert_eq!(
        result.get(&PhpKey::from_bytes(b"minimum")),
        Some(&absint_integer(i64::MIN))
    );
    assert_eq!(
        result.get(&PhpKey::from_bytes(b"maximum")),
        Some(&PhpValue::Integer(i64::MAX))
    );
}

#[test]
fn id_list_does_not_guess_unimplemented_scalar_coercions() {
    for value in [
        PhpValue::Bool(true),
        PhpValue::Float(1.2),
        PhpValue::Bytes(b"1,2".to_vec()),
        PhpValue::Unsupported,
    ] {
        let mut input = PhpArray::new();
        input.insert(PhpKey::Integer(0), value);
        assert_eq!(
            wp_parse_id_list_integers(&input),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
}

#[test]
fn id_list_root_booleans_and_signed_integers_keep_list_cast_before_absint() {
    assert!(wp_parse_id_list(&PhpValue::Bool(false)).unwrap().is_empty());
    for (input, expected) in [
        (PhpValue::Bool(true), PhpValue::Integer(1)),
        (PhpValue::Integer(0), PhpValue::Integer(0)),
        (PhpValue::Integer(-1), PhpValue::Integer(1)),
        (
            PhpValue::Integer(i64::MIN),
            PhpValue::Float(9_223_372_036_854_775_808.0),
        ),
        (PhpValue::Integer(i64::MAX), PhpValue::Integer(i64::MAX)),
    ] {
        let parsed = wp_parse_id_list(&input).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.get(&PhpKey::Integer(0)), Some(&expected));
    }
}

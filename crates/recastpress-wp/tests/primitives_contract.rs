use recastpress_wp::{
    CompatibilityError, sanitize_key_bytes,
    value::{PhpArray, PhpKey, PhpValue},
    wp_array_slice_assoc, wp_is_numeric_array, wp_parse_args_arrays, wp_parse_list,
    wp_validate_boolean,
};

#[test]
fn boolean_validation_keeps_wordpress_string_rules() {
    for bytes in [b"".as_slice(), b"0", b"false", b"FALSE", b"FaLsE"] {
        assert_eq!(
            wp_validate_boolean(&PhpValue::Bytes(bytes.to_vec())),
            Ok(false)
        );
    }
    for bytes in [
        b"true".as_slice(),
        b"off",
        b"no",
        b" false ",
        b"00",
        b"0.0",
        b"\0",
        b"\xff",
    ] {
        assert_eq!(
            wp_validate_boolean(&PhpValue::Bytes(bytes.to_vec())),
            Ok(true)
        );
    }
    assert_eq!(wp_validate_boolean(&PhpValue::Null), Ok(false));
    assert_eq!(wp_validate_boolean(&PhpValue::Bool(true)), Ok(true));
    assert_eq!(wp_validate_boolean(&PhpValue::Integer(0)), Ok(false));
    assert_eq!(wp_validate_boolean(&PhpValue::Integer(-1)), Ok(true));
    assert_eq!(wp_validate_boolean(&PhpValue::Float(-0.0)), Ok(false));
    assert_eq!(
        wp_validate_boolean(&PhpValue::Float(f64::NAN)),
        Err(CompatibilityError::UnsupportedValue)
    );
    assert_eq!(
        wp_validate_boolean(&PhpValue::Unsupported),
        Err(CompatibilityError::UnsupportedValue)
    );
    let mut array = PhpArray::new();
    assert_eq!(
        wp_validate_boolean(&PhpValue::Array(array.clone())),
        Ok(false)
    );
    array.insert(PhpKey::Integer(99), PhpValue::Null);
    assert_eq!(wp_validate_boolean(&PhpValue::Array(array)), Ok(true));
}

#[test]
fn parse_args_arrays_renumbers_only_when_defaults_are_nonempty() {
    let mut defaults = PhpArray::new();
    defaults.insert(
        PhpKey::Integer(-7),
        PhpValue::Bytes(b"default item".to_vec()),
    );
    defaults.insert(PhpKey::Bytes(b"x".to_vec()), PhpValue::Bool(true));
    defaults.insert(PhpKey::Bytes(b"01".to_vec()), PhpValue::Integer(3));
    let mut args = PhpArray::new();
    args.insert(PhpKey::Integer(99), PhpValue::Bytes(b"arg item".to_vec()));
    args.insert(PhpKey::Bytes(b"x".to_vec()), PhpValue::Null);
    args.insert(PhpKey::Bytes(b"y".to_vec()), PhpValue::Bool(false));
    assert_eq!(
        wp_parse_args_arrays(&args, &PhpArray::new()),
        Ok(args.clone())
    );
    let mut expected = PhpArray::new();
    expected.insert(
        PhpKey::Integer(0),
        PhpValue::Bytes(b"default item".to_vec()),
    );
    expected.insert(PhpKey::Bytes(b"x".to_vec()), PhpValue::Null);
    expected.insert(PhpKey::Bytes(b"01".to_vec()), PhpValue::Integer(3));
    expected.insert(PhpKey::Integer(1), PhpValue::Bytes(b"arg item".to_vec()));
    expected.insert(PhpKey::Bytes(b"y".to_vec()), PhpValue::Bool(false));
    assert_eq!(wp_parse_args_arrays(&args, &defaults), Ok(expected));
    args.insert(PhpKey::Integer(5), PhpValue::Unsupported);
    assert_eq!(
        wp_parse_args_arrays(&args, &defaults),
        Err(CompatibilityError::UnsupportedValue)
    );
}

#[test]
fn associative_slice_uses_requested_order_isset_and_canonical_keys() {
    let mut input = PhpArray::new();
    input.insert(PhpKey::Integer(1), PhpValue::Bool(false));
    input.insert(PhpKey::Bytes(b"null".to_vec()), PhpValue::Null);
    input.insert(PhpKey::Bytes(b"x".to_vec()), PhpValue::Bytes(vec![0, 255]));
    let slice = wp_array_slice_assoc(
        &input,
        &[
            PhpKey::Bytes(b"x".to_vec()),
            PhpKey::Bytes(b"null".to_vec()),
            PhpKey::Bytes(b"missing".to_vec()),
            PhpKey::Bytes(b"1".to_vec()),
            PhpKey::Bytes(b"x".to_vec()),
        ],
    )
    .unwrap();
    let mut expected = PhpArray::new();
    expected.insert(PhpKey::Bytes(b"x".to_vec()), PhpValue::Bytes(vec![0, 255]));
    expected.insert(PhpKey::Integer(1), PhpValue::Bool(false));
    assert_eq!(slice, expected);
    input.insert(PhpKey::Integer(5), PhpValue::Unsupported);
    assert_eq!(
        wp_array_slice_assoc(&input, &[PhpKey::Integer(5)]),
        Err(CompatibilityError::UnsupportedValue)
    );
}

#[test]
fn parse_list_splits_bytes_and_filters_arrays_without_reindexing() {
    let parsed = wp_parse_list(&PhpValue::Bytes(
        b" ,a\tb\nc\x0bd\x0ce\rf ,\xff\0\xc2\xa0".to_vec(),
    ))
    .unwrap();
    let values: Vec<_> = parsed.iter().map(|(_, value)| value.clone()).collect();
    assert_eq!(
        values,
        [
            b"a".as_slice(),
            b"b",
            b"c",
            b"d",
            b"e",
            b"f",
            b"\xff\0\xc2\xa0"
        ]
        .map(|bytes| PhpValue::Bytes(bytes.to_vec()))
    );
    let mut input = PhpArray::new();
    input.insert(PhpKey::Integer(-4), PhpValue::Null);
    input.insert(PhpKey::Integer(7), PhpValue::Bool(false));
    input.insert(
        PhpKey::Bytes(b"x".to_vec()),
        PhpValue::Array(PhpArray::new()),
    );
    input.insert(PhpKey::Integer(19), PhpValue::Bytes(b"a,b".to_vec()));
    let mut expected = PhpArray::new();
    expected.insert(PhpKey::Integer(7), PhpValue::Bool(false));
    expected.insert(PhpKey::Integer(19), PhpValue::Bytes(b"a,b".to_vec()));
    assert_eq!(wp_parse_list(&PhpValue::Array(input)), Ok(expected));
    assert_eq!(
        wp_parse_list(&PhpValue::Null),
        Err(CompatibilityError::UnsupportedValue)
    );
    assert!(wp_parse_list(&PhpValue::Bytes(vec![])).unwrap().is_empty());
}

#[test]
fn numeric_array_allows_holes_and_negative_keys_but_not_string_keys() {
    let mut array = PhpArray::new();
    assert!(wp_is_numeric_array(&PhpValue::Array(array.clone())));
    array.insert(PhpKey::Integer(-5), PhpValue::Null);
    array.insert(PhpKey::Integer(99), PhpValue::Bool(false));
    assert!(wp_is_numeric_array(&PhpValue::Array(array.clone())));
    array.insert(PhpKey::Bytes(b"01".to_vec()), PhpValue::Null);
    assert!(!wp_is_numeric_array(&PhpValue::Array(array)));
    assert!(!wp_is_numeric_array(&PhpValue::Bytes(b"array".to_vec())));
    assert!(!wp_is_numeric_array(&PhpValue::Unsupported));
}

#[test]
fn array_keys_coerce_only_canonical_in_range_integer_strings() {
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert_eq!(
            PhpKey::from_bytes(value.to_string().as_bytes()),
            PhpKey::Integer(value)
        );
    }
    for bytes in [
        b"01".as_slice(),
        b"+1",
        b"-0",
        b" 1",
        b"1 ",
        b"1.0",
        b"9223372036854775808",
        b"\xff",
    ] {
        assert_eq!(PhpKey::from_bytes(bytes), PhpKey::Bytes(bytes.to_vec()));
    }
    let mut array = PhpArray::new();
    array.insert(PhpKey::Integer(1), PhpValue::Bytes(b"first".to_vec()));
    array.insert(PhpKey::Integer(9), PhpValue::Null);
    assert!(
        array
            .insert(PhpKey::Bytes(b"1".to_vec()), PhpValue::Bool(true))
            .is_some()
    );
    assert_eq!(array.len(), 2);
    assert_eq!(
        array.iter().next(),
        Some((&PhpKey::Integer(1), &PhpValue::Bool(true)))
    );
}

#[test]
fn sanitize_key_string_kernel_preserves_original_bytes_for_filter() {
    let original = b"UPPER_key-09 <script>\xff\0";
    let mut called = false;
    let result = sanitize_key_bytes(original, |sanitized, raw| {
        called = true;
        assert_eq!(raw, original);
        assert_eq!(sanitized, b"upper_key-09script");
        b"filter override".to_vec()
    });
    assert!(called);
    assert_eq!(result, b"filter override");
    assert_eq!(
        sanitize_key_bytes("日本語Ä".as_bytes(), |clean, _| clean),
        b""
    );
}

#[test]
fn parse_list_root_booleans_and_signed_integers_use_php_string_values() {
    assert!(wp_parse_list(&PhpValue::Bool(false)).unwrap().is_empty());
    for (input, expected) in [
        (PhpValue::Bool(true), b"1".as_slice()),
        (PhpValue::Integer(0), b"0"),
        (PhpValue::Integer(-1), b"-1"),
        (PhpValue::Integer(i64::MIN), b"-9223372036854775808"),
        (PhpValue::Integer(i64::MAX), b"9223372036854775807"),
    ] {
        let result = wp_parse_list(&input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get(&PhpKey::Integer(0)),
            Some(&PhpValue::Bytes(expected.to_vec()))
        );
    }
}

#[test]
fn parse_list_null_diagnostic_and_root_float_formatting_remain_unsupported() {
    for input in [
        PhpValue::Null,
        PhpValue::Float(0.0),
        PhpValue::Float(1.5),
        PhpValue::Float(f64::NAN),
        PhpValue::Float(f64::INFINITY),
        PhpValue::Unsupported,
    ] {
        // PHP 8.5 emits E_DEPRECATED for the root Null passed to preg_split;
        // the native value model must not silently replace that with an empty list.
        assert_eq!(
            wp_parse_list(&input),
            Err(CompatibilityError::UnsupportedValue)
        );
        assert_eq!(
            recastpress_wp::wp_parse_id_list(&input),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
}

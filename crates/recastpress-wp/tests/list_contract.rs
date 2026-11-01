use recastpress_wp::{
    CompatibilityError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_list_pluck,
};

fn row(value: PhpValue, index: Option<PhpValue>) -> PhpValue {
    let mut row = PhpArray::new();
    row.insert(PhpKey::from_bytes(b"value"), value);
    if let Some(index) = index {
        row.insert(PhpKey::from_bytes(b"index"), index);
    }
    PhpValue::Array(row)
}
fn list(rows: Vec<PhpValue>) -> PhpValue {
    let mut list = PhpArray::new();
    for (index, row) in rows.into_iter().enumerate() {
        list.insert(PhpKey::Integer(-10 + index as i64 * 2), row);
    }
    PhpValue::Array(list)
}

#[test]
fn pluck_preserves_original_keys_and_selected_null_with_falsey_index_selector() {
    let input = list(vec![
        row(PhpValue::Null, None),
        row(PhpValue::Bytes(b"a\0\xff".to_vec()), None),
    ]);
    let field = PhpKey::from_bytes(b"value");
    let expected = wp_list_pluck(&input, &field, None).unwrap();
    assert_eq!(
        expected
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        [PhpKey::Integer(-10), PhpKey::Integer(-8)]
    );
    assert_eq!(expected.get(&PhpKey::Integer(-10)), Some(&PhpValue::Null));
    for key in [
        PhpKey::Integer(0),
        PhpKey::Bytes(b"0".to_vec()),
        PhpKey::Bytes(vec![]),
    ] {
        assert_eq!(wp_list_pluck(&input, &field, Some(&key)).unwrap(), expected);
    }
    assert!(
        wp_list_pluck(&PhpValue::Integer(42), &field, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn pluck_indexed_keys_overwrite_in_place_and_append_after_negative_maximum() {
    let input = list(vec![
        row(PhpValue::Integer(1), Some(PhpValue::Integer(-5))),
        row(PhpValue::Integer(2), None),
        row(PhpValue::Integer(3), Some(PhpValue::Null)),
        row(PhpValue::Integer(4), Some(PhpValue::Bytes(b"-5".to_vec()))),
        row(PhpValue::Integer(5), Some(PhpValue::Bytes(b"01".to_vec()))),
        row(PhpValue::Integer(6), Some(PhpValue::Bool(false))),
        row(PhpValue::Integer(7), None),
    ]);
    let output = wp_list_pluck(
        &input,
        &PhpKey::from_bytes(b"value"),
        Some(&PhpKey::from_bytes(b"index")),
    )
    .unwrap();
    assert_eq!(
        output
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>(),
        [
            PhpKey::Integer(-5),
            PhpKey::Integer(-4),
            PhpKey::Integer(-3),
            PhpKey::from_bytes(b"01"),
            PhpKey::Integer(0),
            PhpKey::Integer(1)
        ]
    );
    assert_eq!(
        output.get(&PhpKey::Integer(-5)),
        Some(&PhpValue::Integer(4))
    );
}

#[test]
fn pluck_rejects_unmodeled_warnings_key_coercions_and_append_overflow() {
    let field = PhpKey::from_bytes(b"value");
    let index = PhpKey::from_bytes(b"index");
    for input in [
        PhpValue::Unsupported,
        list(vec![PhpValue::Null]),
        list(vec![PhpValue::Array(PhpArray::new())]),
        list(vec![row(PhpValue::Unsupported, None)]),
        list(vec![row(PhpValue::Null, Some(PhpValue::Float(1.5)))]),
        list(vec![row(
            PhpValue::Null,
            Some(PhpValue::Array(PhpArray::new())),
        )]),
        list(vec![
            row(PhpValue::Null, Some(PhpValue::Integer(i64::MAX))),
            row(PhpValue::Null, None),
        ]),
    ] {
        assert_eq!(
            wp_list_pluck(&input, &field, Some(&index)),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
    let maximum = list(vec![row(PhpValue::Null, Some(PhpValue::Integer(i64::MAX)))]);
    assert!(wp_list_pluck(&maximum, &field, Some(&index)).is_ok());
}

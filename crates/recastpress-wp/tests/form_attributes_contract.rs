use recastpress_wp::{
    CompatibilityError, checked_return, disabled_return, selected_return,
    value::{PhpArray, PhpValue},
    wp_readonly_return,
};

type AttributeFn = fn(&PhpValue, &PhpValue) -> Result<Vec<u8>, CompatibilityError>;

fn functions() -> [(AttributeFn, &'static [u8]); 4] {
    [
        (checked_return, b" checked='checked'"),
        (selected_return, b" selected='selected'"),
        (disabled_return, b" disabled='disabled'"),
        (wp_readonly_return, b" readonly='readonly'"),
    ]
}

fn bytes(value: &[u8]) -> PhpValue {
    PhpValue::Bytes(value.to_vec())
}

#[test]
fn form_attributes_compare_explicit_string_casts_with_exact_fixed_output() {
    let cases = [
        (PhpValue::Null, PhpValue::Null, true),
        (PhpValue::Null, PhpValue::Bool(false), true),
        (PhpValue::Null, bytes(b""), true),
        (PhpValue::Bool(false), bytes(b""), true),
        (PhpValue::Bool(true), bytes(b"1"), true),
        (PhpValue::Bool(true), PhpValue::Integer(1), true),
        (PhpValue::Bool(false), PhpValue::Integer(0), false),
        (PhpValue::Null, bytes(b"0"), false),
        (PhpValue::Integer(0), bytes(b"0"), true),
        (PhpValue::Integer(0), bytes(b""), false),
        (PhpValue::Integer(1), bytes(b"01"), false),
        (PhpValue::Integer(-1), bytes(b"-01"), false),
    ];
    for (function, attribute) in functions() {
        for (left, right, matches) in &cases {
            let expected = if *matches { attribute } else { b"" };
            assert_eq!(function(left, right).unwrap(), expected);
            assert_eq!(function(right, left).unwrap(), expected);
        }
    }
}

#[test]
fn form_attributes_integer_casts_preserve_signed_boundaries_and_no_numeric_comparison() {
    for (function, attribute) in functions() {
        for (number, decimal) in [
            (i64::MIN, b"-9223372036854775808".as_slice()),
            (i64::MAX, b"9223372036854775807".as_slice()),
            (-42, b"-42".as_slice()),
        ] {
            assert_eq!(
                function(&PhpValue::Integer(number), &bytes(decimal)).unwrap(),
                attribute
            );
        }
        for unequal in [b"+1".as_slice(), b"1.0", b"1e0", b" 1", b"1\n"] {
            assert_eq!(
                function(&PhpValue::Integer(1), &bytes(unequal)).unwrap(),
                b""
            );
        }
    }
}

#[test]
fn form_attributes_keep_binary_bytes_exact_and_leave_inputs_unchanged() {
    for (function, attribute) in functions() {
        for byte in 0..=255u8 {
            let left = bytes(&[b'a', byte, b'\0', 0xff]);
            let before = left.clone();
            assert_eq!(function(&left, &before).unwrap(), attribute);
            assert_eq!(function(&left, &bytes(&[b'a', byte])).unwrap(), b"");
            assert_eq!(left, before);
        }
    }
}

#[test]
fn form_attributes_refuse_unsupported_values_on_either_side_even_when_equal() {
    for (function, _) in functions() {
        for unsupported in [
            PhpValue::Float(0.0),
            PhpValue::Float(-0.0),
            PhpValue::Float(f64::NAN),
            PhpValue::Float(f64::INFINITY),
            PhpValue::Array(PhpArray::new()),
            PhpValue::Unsupported,
        ] {
            for (left, right) in [
                (&unsupported, &unsupported),
                (&unsupported, &PhpValue::Null),
                (&PhpValue::Null, &unsupported),
            ] {
                assert_eq!(
                    function(left, right),
                    Err(CompatibilityError::UnsupportedValue)
                );
            }
        }
    }
}

use recastpress_wp::{
    JsonEncodeError, JsonFlags,
    value::{PhpArray, PhpKey, PhpValue},
    wp_json_encode, wp_json_encode_utf8_no_mbstring, wp_json_encode_with_flags,
};

fn list(values: impl IntoIterator<Item = PhpValue>) -> PhpValue {
    let mut array = PhpArray::new();
    for (index, value) in values.into_iter().enumerate() {
        array.insert(PhpKey::Integer(index as i64), value);
    }
    PhpValue::Array(array)
}

#[test]
fn default_json_preserves_del_and_html_bytes_but_escapes_all_controls_slashes_and_quotes() {
    let mut raw: Vec<u8> = (0..32).collect();
    raw.extend_from_slice(b"\"\\/\x7f<>&'");
    let value = PhpValue::Bytes(raw.clone());
    let mut expected = br#""\u0000\u0001\u0002\u0003\u0004\u0005\u0006\u0007\b\t\n\u000b\f\r\u000e\u000f\u0010\u0011\u0012\u0013\u0014\u0015\u0016\u0017\u0018\u0019\u001a\u001b\u001c\u001d\u001e\u001f\"\\\/"#.to_vec();
    expected.extend_from_slice(b"\x7f<>&'\"");
    assert_eq!(wp_json_encode(&value, usize::MAX).unwrap(), expected);
    assert_eq!(value, PhpValue::Bytes(raw));
    fn is_error(_: &dyn std::error::Error) {}
    for error in [
        JsonEncodeError::UnsupportedValue,
        JsonEncodeError::RecursionLimit,
        JsonEncodeError::OutputLimit,
    ] {
        is_error(&error);
        assert!(!error.to_string().is_empty());
        assert!(!error.to_string().contains("<>&"));
    }
}

#[test]
fn unicode_is_lowercase_utf16_without_normalizing_noncharacters_or_line_separators() {
    let raw = "é€日本語\u{2028}\u{2029}🦀\u{10000}\u{10ffff}\u{fffe}\u{ffff}"
        .as_bytes()
        .to_vec();
    let value = PhpValue::Bytes(raw.clone());
    assert_eq!(wp_json_encode(&value, usize::MAX).unwrap(), br#""\u00e9\u20ac\u65e5\u672c\u8a9e\u2028\u2029\ud83e\udd80\ud800\udc00\udbff\udfff\ufffe\uffff""#);
    assert_eq!(value, PhpValue::Bytes(raw));
}

#[test]
fn list_detection_uses_integer_key_order_and_objects_keep_canonicalized_keys_and_order() {
    let values = list([
        PhpValue::Null,
        PhpValue::Bool(false),
        PhpValue::Bool(true),
        PhpValue::Integer(i64::MIN),
        PhpValue::Integer(i64::MAX),
    ]);
    assert_eq!(
        wp_json_encode(&values, usize::MAX).unwrap(),
        b"[null,false,true,-9223372036854775808,9223372036854775807]"
    );
    assert_eq!(wp_json_encode(&list([]), 2).unwrap(), b"[]");
    let mut object = PhpArray::new();
    object.insert(PhpKey::Bytes(b"01".to_vec()), PhpValue::Bool(false));
    object.insert(PhpKey::Bytes(b"0".to_vec()), PhpValue::Null);
    object.insert(PhpKey::Integer(i64::MIN), PhpValue::Integer(-1));
    object.insert(PhpKey::Integer(i64::MAX), PhpValue::Integer(1));
    object.insert(PhpKey::Bytes(b"".to_vec()), PhpValue::Bytes(b"/".to_vec()));
    object.insert(
        PhpKey::Bytes("\0語".as_bytes().to_vec()),
        PhpValue::Bool(true),
    );
    object.insert(PhpKey::Integer(0), PhpValue::Integer(7));
    let original = PhpValue::Array(object);
    let snapshot = original.clone();
    assert_eq!(wp_json_encode(&original, usize::MAX).unwrap(), br#"{"01":false,"0":7,"-9223372036854775808":-1,"9223372036854775807":1,"":"\/","\u0000\u8a9e":true}"#);
    assert_eq!(original, snapshot);
    for (keys, expected) in [
        ([1, 0], b"{\"1\":null,\"0\":null}".as_slice()),
        ([0, 2], b"{\"0\":null,\"2\":null}"),
    ] {
        let mut array = PhpArray::new();
        for key in keys {
            array.insert(PhpKey::Integer(key), PhpValue::Null);
        }
        assert_eq!(
            wp_json_encode(&PhpValue::Array(array), usize::MAX).unwrap(),
            expected
        );
    }
    let mut canonical = PhpArray::new();
    canonical.insert(PhpKey::Bytes(b"0".to_vec()), PhpValue::Bool(true));
    canonical.insert(PhpKey::Bytes(b"1".to_vec()), PhpValue::Bool(false));
    assert_eq!(
        wp_json_encode(&PhpValue::Array(canonical), usize::MAX).unwrap(),
        b"[true,false]"
    );
}

#[test]
fn complete_output_cap_is_exact_and_unsupported_values_or_keys_never_return_partial_json() {
    for (value, expected) in [
        (PhpValue::Null, b"null".as_slice()),
        (PhpValue::Bool(false), b"false"),
        (
            PhpValue::Bytes("🦀/".as_bytes().to_vec()),
            br#""\ud83e\udd80\/""#,
        ),
        (list([]), b"[]"),
    ] {
        let snapshot = value.clone();
        assert_eq!(wp_json_encode(&value, expected.len()).unwrap(), expected);
        for cap in [0, expected.len() - 1] {
            assert_eq!(
                wp_json_encode(&value, cap),
                Err(JsonEncodeError::OutputLimit)
            );
        }
        assert_eq!(value, snapshot);
    }
    for unsupported in [
        PhpValue::Float(0.0),
        PhpValue::Float(f64::NAN),
        PhpValue::Float(f64::INFINITY),
        PhpValue::Unsupported,
        PhpValue::Bytes(vec![0xff]),
        PhpValue::Bytes(vec![0xed, 0xa0, 0x80]),
        PhpValue::Bytes(vec![0xc0, 0x80]),
    ] {
        for value in [
            unsupported.clone(),
            list([PhpValue::Null, unsupported.clone()]),
            list([unsupported, PhpValue::Bool(true)]),
        ] {
            for cap in [0, usize::MAX] {
                assert_eq!(
                    wp_json_encode(&value, cap),
                    Err(JsonEncodeError::UnsupportedValue)
                );
            }
        }
    }
    let mut array = PhpArray::new();
    array.insert(PhpKey::Bytes(vec![0xff]), PhpValue::Null);
    let value = PhpValue::Array(array);
    let snapshot = value.clone();
    assert_eq!(
        wp_json_encode(&value, 0),
        Err(JsonEncodeError::UnsupportedValue)
    );
    assert_eq!(value, snapshot);
}

#[test]
fn host_depth_counts_root_and_empty_arrays_at_64_and_validation_precedes_output_cap() {
    fn nested(mut value: PhpValue, count: usize) -> PhpValue {
        for _ in 0..count {
            value = list([value]);
        }
        value
    }
    let value = nested(PhpValue::Null, 64);
    let snapshot = value.clone();
    let expected = [b"[".repeat(64), b"null".to_vec(), b"]".repeat(64)].concat();
    assert_eq!(wp_json_encode(&value, expected.len()).unwrap(), expected);
    assert_eq!(value, snapshot);
    assert!(wp_json_encode(&nested(list([]), 63), usize::MAX).is_ok());
    for value in [nested(PhpValue::Null, 65), nested(list([]), 64)] {
        for cap in [0, usize::MAX] {
            assert_eq!(
                wp_json_encode(&value, cap),
                Err(JsonEncodeError::RecursionLimit)
            );
        }
    }
    assert_eq!(
        wp_json_encode(&nested(PhpValue::Unsupported, 64), 0),
        Err(JsonEncodeError::UnsupportedValue)
    );
}

const HEX_PREFIXES: [&[u8]; 16] = [
    br#"<>&'\""#,
    br#"\u003C\u003E&'\""#,
    br#"<>\u0026'\""#,
    br#"\u003C\u003E\u0026'\""#,
    br#"<>&\u0027\""#,
    br#"\u003C\u003E&\u0027\""#,
    br#"<>\u0026\u0027\""#,
    br#"\u003C\u003E\u0026\u0027\""#,
    br#"<>&'\u0022"#,
    br#"\u003C\u003E&'\u0022"#,
    br#"<>\u0026'\u0022"#,
    br#"\u003C\u003E\u0026'\u0022"#,
    br#"<>&\u0027\u0022"#,
    br#"\u003C\u003E&\u0027\u0022"#,
    br#"<>\u0026\u0027\u0022"#,
    br#"\u003C\u003E\u0026\u0027\u0022"#,
];

fn selected_flags(combination: u32) -> JsonFlags {
    let bits = [1, 2, 4, 8, 16, 64, 256, 2048]
        .into_iter()
        .enumerate()
        .filter(|(index, _)| combination & (1 << index) != 0)
        .fold(0, |bits, (_, flag)| bits | flag);
    JsonFlags::from_bits(bits).unwrap()
}

#[test]
fn selected_flag_bits_admit_exactly_the_supported_mask_and_preserve_error_traits() {
    assert_eq!(JsonFlags::default().bits(), 0);
    for (flag, bit) in [
        (JsonFlags::HEX_TAG, 1),
        (JsonFlags::HEX_AMP, 2),
        (JsonFlags::HEX_APOS, 4),
        (JsonFlags::HEX_QUOT, 8),
        (JsonFlags::FORCE_OBJECT, 16),
        (JsonFlags::UNESCAPED_SLASHES, 64),
        (JsonFlags::PRETTY_PRINT, 128),
        (JsonFlags::UNESCAPED_UNICODE, 256),
        (JsonFlags::UNESCAPED_LINE_TERMINATORS, 2048),
    ] {
        assert_eq!(flag.bits(), bit);
    }
    for combination in 0..256 {
        let flags = selected_flags(combination);
        assert_eq!(JsonFlags::from_bits(flags.bits()), Ok(flags));
    }
    assert_eq!(selected_flags(255).bits(), 2399);
    for shift in 0..32 {
        let bit = 1u32 << shift;
        if bit & 2527 == 0 {
            assert_eq!(
                JsonFlags::from_bits(bit),
                Err(JsonEncodeError::UnsupportedFlags)
            );
            assert_eq!(
                JsonFlags::from_bits(bit | 2527),
                Err(JsonEncodeError::UnsupportedFlags)
            );
        }
    }
    assert_eq!(
        JsonFlags::from_bits(u32::MAX),
        Err(JsonEncodeError::UnsupportedFlags)
    );
    let error = JsonEncodeError::UnsupportedFlags;
    let object: &dyn std::error::Error = &error;
    assert_eq!(
        object.to_string(),
        "JSON flags are outside the supported flag subset"
    );
}

#[test]
fn hex_flag_combinations_keep_php_uppercase_tag_digits_and_independent_other_escapes() {
    let value = PhpValue::Bytes(b"<>&'\"".to_vec());
    for (bits, prefix) in HEX_PREFIXES.iter().enumerate() {
        let expected = [b"\"".as_slice(), *prefix, b"\""].concat();
        assert_eq!(
            wp_json_encode_with_flags(
                &value,
                JsonFlags::from_bits(bits as u32).unwrap(),
                usize::MAX
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn unicode_and_line_terminator_flags_interact_without_changing_slash_policy() {
    let value = PhpValue::Bytes("é🦀\u{2028}\u{2029}/".as_bytes().to_vec());
    for (bits, expected) in [
        (0, br#""\u00e9\ud83e\udd80\u2028\u2029\/""#.as_slice()),
        (64, br#""\u00e9\ud83e\udd80\u2028\u2029/""#),
        (256, "\"é🦀\\u2028\\u2029\\/\"".as_bytes()),
        (320, "\"é🦀\\u2028\\u2029/\"".as_bytes()),
        (2048, br#""\u00e9\ud83e\udd80\u2028\u2029\/""#),
        (2112, br#""\u00e9\ud83e\udd80\u2028\u2029/""#),
        (2304, "\"é🦀\u{2028}\u{2029}\\/\"".as_bytes()),
        (2368, "\"é🦀\u{2028}\u{2029}/\"".as_bytes()),
    ] {
        assert_eq!(
            wp_json_encode_with_flags(&value, JsonFlags::from_bits(bits).unwrap(), usize::MAX)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn force_object_applies_to_empty_and_nested_lists_without_reordering_keys() {
    let mut object = PhpArray::new();
    object.insert(PhpKey::Bytes(b"k".to_vec()), list([]));
    let value = list([
        list([]),
        list([PhpValue::Bool(true)]),
        PhpValue::Array(object),
    ]);
    let snapshot = value.clone();
    assert_eq!(
        wp_json_encode(&value, usize::MAX).unwrap(),
        br#"[[],[true],{"k":[]}]"#
    );
    assert_eq!(
        wp_json_encode_with_flags(&value, JsonFlags::FORCE_OBJECT, usize::MAX).unwrap(),
        br#"{"0":{},"1":{"0":true},"2":{"k":{}}}"#
    );
    assert_eq!(
        wp_json_encode_with_flags(&list([]), JsonFlags::FORCE_OBJECT, 2).unwrap(),
        b"{}"
    );
    assert_eq!(value, snapshot);
}

#[test]
fn every_selected_combination_uses_same_flags_in_keys_and_values_and_retains_controls() {
    let value = list([
        PhpValue::Bytes("<>&'\"/é\u{2028}".as_bytes().to_vec()),
        PhpValue::Bytes(b"\\\n\t\0".to_vec()),
    ]);
    let snapshot = value.clone();
    for combination in 0..256 {
        let flags = selected_flags(combination);
        let bits = flags.bits();
        let mut expected = if bits & 16 != 0 {
            b"{\"0\":\"".to_vec()
        } else {
            b"[\"".to_vec()
        };
        expected.extend_from_slice(HEX_PREFIXES[(bits & 15) as usize]);
        expected.extend_from_slice(if bits & 64 != 0 { b"/" } else { br"\/" });
        expected.extend_from_slice(if bits & 256 != 0 {
            "é".as_bytes()
        } else {
            br"\u00e9"
        });
        expected.extend_from_slice(if bits & 2304 == 2304 {
            "\u{2028}".as_bytes()
        } else {
            br"\u2028"
        });
        expected.extend_from_slice(if bits & 16 != 0 {
            br#"","1":"\\\n\t\u0000"}"#
        } else {
            br#"","\\\n\t\u0000"]"#
        });
        assert_eq!(
            wp_json_encode_with_flags(&value, flags, expected.len()).unwrap(),
            expected,
            "bits={bits}"
        );
        assert_eq!(
            wp_json_encode_with_flags(&value, flags, expected.len() - 1),
            Err(JsonEncodeError::OutputLimit)
        );
    }
    assert_eq!(value, snapshot);
    let raw = "<>&'\"/\\\n\0é\u{2028}\u{2029}".as_bytes().to_vec();
    let mut object = PhpArray::new();
    object.insert(PhpKey::Bytes(raw.clone()), PhpValue::Bytes(raw));
    let encoded_string = [
        br#""\u003C\u003E\u0026\u0027\u0022/\\\n\u0000"#.as_slice(),
        "é\u{2028}\u{2029}".as_bytes(),
        b"\"",
    ]
    .concat();
    let expected = [
        b"{".as_slice(),
        &encoded_string,
        b":",
        &encoded_string,
        b"}",
    ]
    .concat();
    assert_eq!(
        wp_json_encode_with_flags(&PhpValue::Array(object), selected_flags(255), usize::MAX)
            .unwrap(),
        expected
    );
}

#[test]
fn flags_zero_wrapper_parity_does_not_expand_domains_or_host_depth() {
    for value in [
        PhpValue::Null,
        PhpValue::Integer(i64::MIN),
        PhpValue::Bytes("/é\u{2028}".as_bytes().to_vec()),
        list([list([])]),
    ] {
        for cap in [0, 1, 128, usize::MAX] {
            assert_eq!(
                wp_json_encode(&value, cap),
                wp_json_encode_with_flags(&value, JsonFlags::default(), cap)
            );
        }
    }
    for flags in [JsonFlags::default(), selected_flags(255)] {
        for value in [
            PhpValue::Float(1.0),
            PhpValue::Float(f64::NAN),
            PhpValue::Unsupported,
            PhpValue::Bytes(vec![0xff]),
            list([PhpValue::Bytes(vec![0xff])]),
        ] {
            assert_eq!(
                wp_json_encode_with_flags(&value, flags, 0),
                Err(JsonEncodeError::UnsupportedValue)
            );
        }
        let mut deep = PhpValue::Null;
        for _ in 0..64 {
            deep = list([deep]);
        }
        assert!(wp_json_encode_with_flags(&deep, flags, usize::MAX).is_ok());
        assert_eq!(
            wp_json_encode_with_flags(&list([deep]), flags, 0),
            Err(JsonEncodeError::RecursionLimit)
        );
    }
}

#[test]
fn pretty_empty_containers_and_scalars_stay_compact_while_nested_nonempty_lists_indent() {
    for (value, expected) in [
        (PhpValue::Null, b"null".as_slice()),
        (PhpValue::Bool(false), b"false"),
        (PhpValue::Integer(i64::MIN), b"-9223372036854775808"),
        (PhpValue::Bytes(b"x/y".to_vec()), br#""x\/y""#),
        (list([]), b"[]"),
    ] {
        assert_eq!(
            wp_json_encode_with_flags(&value, JsonFlags::PRETTY_PRINT, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            wp_json_encode_with_flags(&value, JsonFlags::PRETTY_PRINT, expected.len() - 1),
            Err(JsonEncodeError::OutputLimit)
        );
    }
    let value = list([
        list([]),
        list([PhpValue::Bool(true), PhpValue::Integer(7)]),
        PhpValue::Bytes("é/".as_bytes().to_vec()),
    ]);
    let snapshot = value.clone();
    let expected = b"[\n    [],\n    [\n        true,\n        7\n    ],\n    \"\\u00e9\\/\"\n]";
    assert_eq!(
        wp_json_encode_with_flags(&value, JsonFlags::PRETTY_PRINT, expected.len()).unwrap(),
        expected
    );
    let forced = JsonFlags::from_bits(144).unwrap();
    let expected = b"{\n    \"0\": {},\n    \"1\": {\n        \"0\": true,\n        \"1\": 7\n    },\n    \"2\": \"\\u00e9\\/\"\n}";
    assert_eq!(
        wp_json_encode_with_flags(&value, forced, expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        wp_json_encode_with_flags(&list([]), forced, 2).unwrap(),
        b"{}"
    );
    assert_eq!(value, snapshot);
}

#[test]
fn pretty_all_flags_preserve_key_order_hex_unicode_line_and_nested_forced_objects() {
    let mut nested = PhpArray::new();
    nested.insert(PhpKey::Integer(1), PhpValue::Bytes(b"one".to_vec()));
    nested.insert(
        PhpKey::Integer(0),
        PhpValue::Bytes("é\u{2029}/".as_bytes().to_vec()),
    );
    let mut object = PhpArray::new();
    object.insert(PhpKey::Bytes(b"z".to_vec()), list([]));
    object.insert(
        PhpKey::Bytes("<>&'\"/é\u{2028}".as_bytes().to_vec()),
        PhpValue::Array(nested),
    );
    let value = PhpValue::Array(object);
    let snapshot = value.clone();
    let expected = "{\n    \"z\": {},\n    \"\\u003C\\u003E\\u0026\\u0027\\u0022/é\u{2028}\": {\n        \"1\": \"one\",\n        \"0\": \"é\u{2029}/\"\n    }\n}".as_bytes();
    assert_eq!(
        wp_json_encode_with_flags(&value, JsonFlags::from_bits(2527).unwrap(), expected.len())
            .unwrap(),
        expected
    );
    assert_eq!(
        wp_json_encode_with_flags(
            &value,
            JsonFlags::from_bits(2527).unwrap(),
            expected.len() - 1
        ),
        Err(JsonEncodeError::OutputLimit)
    );
    assert_eq!(value, snapshot);
}

#[test]
fn pretty_pairs_with_every_existing_combination_and_caps_include_escaping_and_indent() {
    let value = list([
        PhpValue::Bytes("<>&'\"/é\u{2028}".as_bytes().to_vec()),
        PhpValue::Bytes(b"\\\n\t\0".to_vec()),
    ]);
    for combination in 0..256 {
        let bits = selected_flags(combination).bits() | JsonFlags::PRETTY_PRINT.bits();
        let flags = JsonFlags::from_bits(bits).unwrap();
        let mut expected = if bits & 16 != 0 {
            b"{\n    \"0\": \"".to_vec()
        } else {
            b"[\n    \"".to_vec()
        };
        expected.extend_from_slice(HEX_PREFIXES[(bits & 15) as usize]);
        expected.extend_from_slice(if bits & 64 != 0 { b"/" } else { br"\/" });
        expected.extend_from_slice(if bits & 256 != 0 {
            "é".as_bytes()
        } else {
            br"\u00e9"
        });
        expected.extend_from_slice(if bits & 2304 == 2304 {
            "\u{2028}".as_bytes()
        } else {
            br"\u2028"
        });
        expected.extend_from_slice(if bits & 16 != 0 {
            b"\",\n    \"1\": \"\\\\\\n\\t\\u0000\"\n}"
        } else {
            b"\",\n    \"\\\\\\n\\t\\u0000\"\n]"
        });
        assert_eq!(
            wp_json_encode_with_flags(&value, flags, expected.len()).unwrap(),
            expected,
            "bits={bits}"
        );
        for cap in [0, expected.len() - 1] {
            assert_eq!(
                wp_json_encode_with_flags(&value, flags, cap),
                Err(JsonEncodeError::OutputLimit)
            );
        }
    }
    assert_eq!(JsonFlags::from_bits(2527).unwrap().bits(), 2527);
}

#[test]
fn native_pretty_host_64_containers_have_checked_indentation_and_do_not_expand_domains() {
    // This native host-boundary test is independent of the PHP value transport's
    // 24-level corpus bound. It is not a 64-level transport comparison.
    let flags = JsonFlags::from_bits(2527).unwrap();
    let mut value = PhpValue::Null;
    for _ in 0..64 {
        value = list([value]);
    }
    let snapshot = value.clone();
    let encoded = wp_json_encode_with_flags(&value, JsonFlags::PRETTY_PRINT, 16644).unwrap();
    assert_eq!(encoded.len(), 16644);
    assert!(encoded.starts_with(b"[\n    [\n        ["));
    assert!(encoded.ends_with(b"]\n    ]\n]"));
    assert_eq!(
        encoded.split(|&byte| byte == b'\n').map(<[u8]>::len).max(),
        Some(260)
    );
    assert_eq!(
        wp_json_encode_with_flags(&value, JsonFlags::PRETTY_PRINT, 16643),
        Err(JsonEncodeError::OutputLimit)
    );
    assert_eq!(value, snapshot);
    assert_eq!(
        wp_json_encode_with_flags(&list([value]), flags, 0),
        Err(JsonEncodeError::RecursionLimit)
    );
    for unsupported in [
        PhpValue::Unsupported,
        PhpValue::Float(0.0),
        PhpValue::Bytes(vec![0xff]),
    ] {
        assert_eq!(
            wp_json_encode_with_flags(&list([PhpValue::Null, unsupported]), flags, 0),
            Err(JsonEncodeError::UnsupportedValue)
        );
    }
}

#[test]
fn opt_in_profile_replaces_maximal_subparts_without_changing_existing_api_rejection() {
    for (raw, expected) in [
        (b"\xff".as_slice(), br#""\ufffd""#.as_slice()),
        (b"\xed\xa0\x80", br#""\ufffd\ufffd\ufffd""#),
        (b"a\xf0\x90\x80", br#""a\ufffd""#),
        (b"\xc0\xaf", br#""\ufffd\ufffd""#),
        (b"a\xe2\x8cZ\0", br#""a\ufffdZ\u0000""#),
    ] {
        let value = PhpValue::Bytes(raw.to_vec());
        let snapshot = value.clone();
        assert_eq!(
            wp_json_encode(&value, usize::MAX),
            Err(JsonEncodeError::UnsupportedValue)
        );
        assert_eq!(
            wp_json_encode_with_flags(&value, JsonFlags::default(), usize::MAX),
            Err(JsonEncodeError::UnsupportedValue)
        );
        assert_eq!(
            wp_json_encode_utf8_no_mbstring(&value, JsonFlags::default(), expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            wp_json_encode_utf8_no_mbstring(&value, JsonFlags::default(), expected.len() - 1),
            Err(JsonEncodeError::OutputLimit)
        );
        assert_eq!(value, snapshot);
    }
    let value = PhpValue::Bytes(b"\xff<>&'\"/".to_vec());
    assert_eq!(
        wp_json_encode_utf8_no_mbstring(&value, JsonFlags::from_bits(2527).unwrap(), usize::MAX)
            .unwrap(),
        "\"�\\u003C\\u003E\\u0026\\u0027\\u0022/\"".as_bytes()
    );
}

#[test]
fn scrubbed_key_collisions_keep_first_position_last_value_and_numeric_normalization() {
    let mut array = PhpArray::new();
    array.insert(
        PhpKey::Bytes(vec![0xff]),
        PhpValue::Bytes(b"first".to_vec()),
    );
    array.insert(PhpKey::Bytes(b"middle".to_vec()), PhpValue::Bool(true));
    array.insert(
        PhpKey::Bytes(vec![0xfe]),
        PhpValue::Bytes(b"second".to_vec()),
    );
    array.insert(
        PhpKey::Bytes("�".as_bytes().to_vec()),
        PhpValue::Bytes(b"literal".to_vec()),
    );
    array.insert(PhpKey::Bytes(b"0".to_vec()), PhpValue::Null);
    array.insert(PhpKey::Bytes(b"00".to_vec()), PhpValue::Integer(2));
    array.insert(PhpKey::Bytes(b"-0".to_vec()), PhpValue::Integer(3));
    array.insert(PhpKey::Bytes(b"0\xff".to_vec()), PhpValue::Integer(4));
    array.insert(PhpKey::Integer(0), PhpValue::Integer(7));
    let value = PhpValue::Array(array);
    let snapshot = value.clone();
    assert_eq!(
        wp_json_encode_utf8_no_mbstring(&value, JsonFlags::default(), usize::MAX).unwrap(),
        br#"{"\ufffd":"literal","middle":true,"0":7,"00":2,"-0":3,"0\ufffd":4}"#
    );
    assert_eq!(value, snapshot);
}

#[test]
fn nested_sanity_and_all_selected_flags_match_a_static_clean_graph_and_valid_fast_path() {
    let mut raw_inner = PhpArray::new();
    raw_inner.insert(
        PhpKey::Bytes(b"x\xff".to_vec()),
        PhpValue::Bytes(b"first".to_vec()),
    );
    raw_inner.insert(PhpKey::Bytes(b"middle".to_vec()), list([]));
    raw_inner.insert(
        PhpKey::Bytes(b"x\xfe".to_vec()),
        PhpValue::Bytes(b"last".to_vec()),
    );
    let raw = list([
        PhpValue::Array(raw_inner),
        PhpValue::Bytes("語/\u{2028}".as_bytes().to_vec()),
        PhpValue::Bytes(vec![0xff]),
    ]);
    let mut clean_inner = PhpArray::new();
    clean_inner.insert(
        PhpKey::Bytes("x�".as_bytes().to_vec()),
        PhpValue::Bytes(b"last".to_vec()),
    );
    clean_inner.insert(PhpKey::Bytes(b"middle".to_vec()), list([]));
    let clean = list([
        PhpValue::Array(clean_inner),
        PhpValue::Bytes("語/\u{2028}".as_bytes().to_vec()),
        PhpValue::Bytes("�".as_bytes().to_vec()),
    ]);
    let snapshot = raw.clone();
    for combination in 0..512 {
        let bits = [1, 2, 4, 8, 16, 64, 128, 256, 2048]
            .into_iter()
            .enumerate()
            .filter(|(index, _)| combination & (1 << index) != 0)
            .fold(0, |bits, (_, bit)| bits | bit);
        let flags = JsonFlags::from_bits(bits).unwrap();
        let expected = wp_json_encode_with_flags(&clean, flags, usize::MAX).unwrap();
        assert_eq!(
            wp_json_encode_utf8_no_mbstring(&raw, flags, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            wp_json_encode_utf8_no_mbstring(&raw, flags, 0),
            Err(JsonEncodeError::OutputLimit)
        );
        for cap in [0, expected.len(), usize::MAX] {
            assert_eq!(
                wp_json_encode_utf8_no_mbstring(&clean, flags, cap),
                wp_json_encode_with_flags(&clean, flags, cap)
            );
        }
    }
    assert_eq!(raw, snapshot);
}

#[test]
fn original_graph_domains_are_validated_before_collisions_can_discard_unsupported_values() {
    for unsupported in [
        PhpValue::Float(0.0),
        PhpValue::Float(f64::NAN),
        PhpValue::Unsupported,
    ] {
        let mut array = PhpArray::new();
        array.insert(PhpKey::Bytes(vec![0xff]), unsupported);
        array.insert(PhpKey::Bytes(vec![0xfe]), PhpValue::Integer(7));
        for cap in [0, usize::MAX] {
            assert_eq!(
                wp_json_encode_utf8_no_mbstring(
                    &PhpValue::Array(array.clone()),
                    JsonFlags::default(),
                    cap
                ),
                Err(JsonEncodeError::UnsupportedValue)
            );
        }
    }
    let value = list([PhpValue::Bytes(vec![0xff]), PhpValue::Unsupported]);
    assert_eq!(
        wp_json_encode_utf8_no_mbstring(&value, JsonFlags::default(), 0),
        Err(JsonEncodeError::UnsupportedValue)
    );
}

#[test]
fn host_depth_is_checked_on_original_graph_and_scalar_leaves_cost_no_extra_level() {
    let mut value = PhpValue::Bytes(vec![0xff]);
    for _ in 0..64 {
        value = list([value]);
    }
    let expected = [b"[".repeat(64), br#""\ufffd""#.to_vec(), b"]".repeat(64)].concat();
    assert_eq!(
        wp_json_encode_utf8_no_mbstring(&value, JsonFlags::default(), expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        wp_json_encode_utf8_no_mbstring(&value, JsonFlags::default(), expected.len() - 1),
        Err(JsonEncodeError::OutputLimit)
    );
    let mut array = PhpArray::new();
    array.insert(PhpKey::Bytes(vec![0xff]), value);
    array.insert(PhpKey::Bytes(vec![0xfe]), PhpValue::Integer(7));
    assert_eq!(
        wp_json_encode_utf8_no_mbstring(&PhpValue::Array(array), JsonFlags::default(), 0),
        Err(JsonEncodeError::RecursionLimit)
    );
}

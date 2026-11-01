use recastpress_wp::{
    maybe_hash_hex_color_bytes, sanitize_hex_color_bytes, sanitize_hex_color_no_hash_bytes,
};

#[test]
fn hex_color_keeps_php_null_distinct_from_empty_and_preserves_case() {
    assert_eq!(sanitize_hex_color_bytes(b""), Some(Vec::new()));
    for color in [b"#aBc".as_slice(), b"#aBc012", b"#000", b"#000000"] {
        assert_eq!(sanitize_hex_color_bytes(color), Some(color.to_vec()));
    }
    for color in [
        b"#".as_slice(),
        b"abc",
        b"#ab",
        b"#abcd",
        b"#abcde",
        b"#abcdef0",
        b"#abcd1234",
        b"red",
        b"transparent",
    ] {
        assert_eq!(sanitize_hex_color_bytes(color), None);
    }
}

#[test]
fn hex_color_accepts_one_final_lf_without_trimming_any_other_bytes() {
    for color in [b"#abc\n".as_slice(), b"#ABC123\n"] {
        assert_eq!(sanitize_hex_color_bytes(color), Some(color.to_vec()));
        assert_eq!(
            sanitize_hex_color_no_hash_bytes(color),
            Some(color[1..].to_vec())
        );
        assert_eq!(maybe_hash_hex_color_bytes(color), color);
    }
    for color in [
        b"#abc\n\n".as_slice(),
        b"#abc\r\n",
        b"#abc\r",
        b"#abc\0",
        b"#abc ",
        b" #abc",
        b"#a\nbc",
    ] {
        assert_eq!(sanitize_hex_color_bytes(color), None);
        assert_eq!(sanitize_hex_color_no_hash_bytes(color), None);
        assert_eq!(maybe_hash_hex_color_bytes(color), color);
    }
}

#[test]
fn no_hash_strips_all_leading_hashes_but_maybe_hash_keeps_false_or_invalid_inputs() {
    for color in [b"".as_slice(), b"#", b"###"] {
        assert_eq!(sanitize_hex_color_no_hash_bytes(color), Some(Vec::new()));
        assert_eq!(maybe_hash_hex_color_bytes(color), color);
    }
    for color in [b"abc".as_slice(), b"#abc", b"###abc"] {
        assert_eq!(
            sanitize_hex_color_no_hash_bytes(color),
            Some(b"abc".to_vec())
        );
        assert_eq!(maybe_hash_hex_color_bytes(color), b"#abc");
    }
    assert_eq!(maybe_hash_hex_color_bytes(b"000"), b"#000");
    assert_eq!(maybe_hash_hex_color_bytes(b"###AbC\n"), b"#AbC\n");
    for color in [b"0".as_slice(), b"#0", b"###invalid", b"a#bc", b"\xff\0"] {
        assert_eq!(sanitize_hex_color_no_hash_bytes(color), None);
        assert_eq!(maybe_hash_hex_color_bytes(color), color);
    }
}

#[test]
fn hex_color_checks_all_byte_values_at_each_digit_position_without_mutation() {
    for template in [b"#aBc".as_slice(), b"#aBc012"] {
        for position in 1..template.len() {
            for byte in 0..=255u8 {
                let mut color = template.to_vec();
                color[position] = byte;
                let before = color.clone();
                let valid = b"0123456789abcdefABCDEF".contains(&byte);
                assert_eq!(
                    sanitize_hex_color_bytes(&color),
                    valid.then(|| color.clone())
                );
                assert_eq!(
                    sanitize_hex_color_no_hash_bytes(&color),
                    valid.then(|| color[1..].to_vec())
                );
                assert_eq!(maybe_hash_hex_color_bytes(&color), color);
                assert_eq!(color, before);
            }
        }
    }
}

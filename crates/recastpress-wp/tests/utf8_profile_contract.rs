use recastpress_wp::wp_check_invalid_utf8_utf8_no_mbstring_bytes;

#[test]
fn empty_and_valid_utf8_preserve_all_bytes_for_both_strip_values() {
    let inputs: &[&[u8]] = &[
        b"",
        b"ASCII\0\x7f\r\n",
        "\u{feff}é日本語😀\u{fdd0}\u{ffff}\u{10ffff}".as_bytes(),
    ];
    for &input in inputs {
        let original = input.to_vec();
        for strip in [false, true] {
            assert_eq!(
                wp_check_invalid_utf8_utf8_no_mbstring_bytes(input, strip),
                original
            );
            assert_eq!(input, original);
        }
    }
}

#[test]
fn invalid_utf8_is_empty_or_maximal_subpart_replacement_without_input_mutation() {
    let cases: &[(&[u8], &str)] = &[
        (b"a\xe2\x8cZ", "a�Z"),
        (b"\xe2\x8c", "�"),
        (b"\xc0\xaf", "��"),
        (b"\xed\xa0\x80", "���"),
        (b"\xf4\x90\x80\x80", "����"),
        (b"\0\x80\xff\xf0\x90\x80", "\0���"),
    ];
    for &(input, expected) in cases {
        let original = input.to_vec();
        assert!(wp_check_invalid_utf8_utf8_no_mbstring_bytes(input, false).is_empty());
        assert_eq!(
            wp_check_invalid_utf8_utf8_no_mbstring_bytes(input, true),
            expected.as_bytes()
        );
        assert_eq!(input, original);
    }
}

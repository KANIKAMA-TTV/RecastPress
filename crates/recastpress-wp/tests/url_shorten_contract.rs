use recastpress_wp::url_shorten_bytes;

#[test]
fn replacements_are_ordered_case_sensitive_everywhere_and_nonrecursive() {
    for (input, expected) in [
        (
            b"https://www.example.test/path/\\//".as_slice(),
            b"example.test/path".as_slice(),
        ),
        (b"ahttps://bhttp://cwww.dhttps://e", b"abcde"),
        (
            b"HTTPS://HTTP://WWW.example/",
            b"HTTPS://HTTP://WWW.example",
        ),
        // The first pass creates the next pass's pattern.
        (b"hthttps://tp://www.x", b"x"),
        // Later passes do not revisit patterns belonging to an earlier pass.
        (b"htwww.tps://x", b"https://x"),
        // A newly formed https:// in its own pass is not removed recursively.
        (b"https:/https:///", b"https:"),
        (b"/\\//", b""),
    ] {
        let original = input.to_vec();
        assert_eq!(url_shorten_bytes(input, 100).unwrap(), expected);
        assert_eq!(input, original);
    }
}

#[test]
fn cutoff_uses_bytes_and_the_literal_entity_at_exact_boundaries() {
    assert_eq!(url_shorten_bytes(b"abc", 3).unwrap(), b"abc");
    assert_eq!(url_shorten_bytes(b"abcd", 3).unwrap(), b"&hellip;");
    assert_eq!(url_shorten_bytes(b"abcd/\\", 4).unwrap(), b"abcd");
    assert_eq!(
        url_shorten_bytes(b"\xff\xfe\0xyz", 5).unwrap(),
        b"\xff\xfe&hellip;"
    );
    assert_eq!(
        url_shorten_bytes("éabcd".as_bytes(), 4).unwrap(),
        b"\xc3&hellip;"
    );
    let input = b"01234567890123456789012345678901234567";
    let mut expected = input[..32].to_vec();
    expected.extend_from_slice(b"&hellip;");
    assert_eq!(url_shorten_bytes(input, 35).unwrap(), expected);
    assert_eq!(url_shorten_bytes(&input[..35], 35).unwrap(), &input[..35]);
    assert_eq!(
        url_shorten_bytes(b"https://www.x/\\", i64::MAX).unwrap(),
        b"x"
    );
}

#[test]
fn short_and_negative_lengths_preserve_php_substring_rules() {
    for (input, length, expected) in [
        (b"".as_slice(), -1, b"&hellip;".as_slice()),
        (b"", 0, b""),
        (b"a", 1, b"a"),
        (b"ab", 2, b"ab"),
        (b"abcd", 0, b"a&hellip;"),
        (b"abcd", 1, b"ab&hellip;"),
        (b"abcd", 2, b"abc&hellip;"),
        (b"abcdefgh", -1, b"abcd&hellip;"),
        (b"abcdefgh", -4, b"a&hellip;"),
        (b"abcdefgh", -100, b"&hellip;"),
    ] {
        assert_eq!(url_shorten_bytes(input, length).unwrap(), expected);
    }
}

#[test]
fn bottom_integer_subtraction_does_not_overflow_or_invent_a_warning() {
    for length in [i64::MIN, i64::MIN + 1, i64::MIN + 2, i64::MIN + 3] {
        for input in [b"".as_slice(), b"https://www.x/", b"\xff\0"] {
            assert_eq!(url_shorten_bytes(input, length).unwrap(), b"&hellip;");
        }
    }
}

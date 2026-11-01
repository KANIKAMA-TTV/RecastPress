use recastpress_wp::{
    SanitizeUserError, sanitize_user_utf8_no_open_angle_empty_filters_bytes as sanitize,
};

#[test]
fn percent_and_shortest_entity_passes_are_ordered_nonrecursive_and_do_not_cross_lf() {
    for (raw, expected) in [
        (b"%4%30A".as_slice(), b"%4A".as_slice()),
        (b"%&x;61", b"%61"),
        (b"&;", b"&;"),
        (b"&;;", b""),
        (b"a&x;&y;b", b"ab"),
        (b"&a\n;b", b"&a ;b"),
        (b"x&a\r;b", b"xb"),
        (b"&&;", b""),
    ] {
        assert_eq!(
            sanitize(raw, false, b"en_US", usize::MAX).unwrap(),
            expected
        );
    }
    let once = sanitize(b"%4%30A", false, b"en_US", usize::MAX).unwrap();
    assert_eq!(sanitize(&once, false, b"en_US", 0), Ok(Vec::new()));
    assert_eq!(
        sanitize("l&x;·l".as_bytes(), false, b"ca", usize::MAX).unwrap(),
        "l·l".as_bytes()
    );
    assert_eq!(
        sanitize("l·l".as_bytes(), false, b"ca", usize::MAX).unwrap(),
        b"ll"
    );
}

#[test]
fn strip_precedes_utf8_admission_and_can_repair_split_sequences_without_latin1_fallback() {
    assert_eq!(
        sanitize(b"\xc3\0\xa9", false, b"en_US", 1),
        Ok(b"e".to_vec())
    );
    for raw in [b"\xff".as_slice(), b"\xc3", b"\xed\xa0\x80"] {
        assert_eq!(
            sanitize(raw, false, b"en_US", 0),
            Err(SanitizeUserError::UnsupportedValue)
        );
    }
    assert_eq!(
        sanitize(b"<", false, b"en_US", 0),
        Err(SanitizeUserError::UnsupportedValue)
    );
    for locale in [b"".as_slice(), b"0"] {
        assert_eq!(
            sanitize(b"", false, locale, 0),
            Err(SanitizeUserError::UnsupportedValue)
        );
    }
}

#[test]
fn strict_ascii_preserves_case_and_non_strict_retains_combining_and_unmapped_unicode() {
    let raw = " É e\u{301} 日本語@_.- A+Z ";
    let saved = raw.as_bytes().to_vec();
    assert_eq!(
        sanitize(raw.as_bytes(), false, b"en_US", usize::MAX).unwrap(),
        "E e\u{301} 日本語@_.- A+Z".as_bytes()
    );
    assert_eq!(
        sanitize(raw.as_bytes(), true, b"en_US", usize::MAX).unwrap(),
        b"E e @_.- AZ"
    );
    assert_eq!(raw.as_bytes(), saved);
}

#[test]
fn trim_precedes_c_six_whitespace_collapse_so_formfeed_can_generate_edge_spaces() {
    assert_eq!(sanitize(b"\x0c", false, b"en_US", 1), Ok(b" ".to_vec()));
    assert_eq!(
        sanitize(b"\x0c A\x0c", false, b"en_US", 3),
        Ok(b" A ".to_vec())
    );
    assert_eq!(
        sanitize(b"\x0c A\x0c", true, b"en_US", 1),
        Ok(b"A".to_vec())
    );
    assert_eq!(
        sanitize(b"A\x0b\x0c\r\n\t B", false, b"en_US", 3),
        Ok(b"A B".to_vec())
    );
}

#[test]
fn only_final_output_is_capped_after_unpooled_intermediates_and_all_removal_stages() {
    let input = "£%41%42&name;".as_bytes();
    assert_eq!(sanitize(input, false, b"en_US", 0), Ok(Vec::new()));
    assert_eq!(
        sanitize("日本語".as_bytes(), true, b"en_US", 0),
        Ok(Vec::new())
    );
    assert_eq!(sanitize(b"A%41B", false, b"en_US", 2), Ok(b"AB".to_vec()));
    assert_eq!(
        sanitize(b"A%41B", false, b"en_US", 1),
        Err(SanitizeUserError::OutputLimit)
    );
    assert_eq!(
        sanitize(b"\x0c", false, b"en_US", 0),
        Err(SanitizeUserError::OutputLimit)
    );
}

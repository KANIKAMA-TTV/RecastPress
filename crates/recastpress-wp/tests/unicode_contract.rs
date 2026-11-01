use recastpress_wp::valid_unicode_i64;

#[test]
fn fixed_character_ranges_and_control_exceptions_keep_their_exact_boundaries() {
    for (value, expected) in [
        (i64::MIN, false),
        (-1, false),
        (0, false),
        (8, false),
        (9, true),
        (10, true),
        (11, false),
        (12, false),
        (13, true),
        (14, false),
        (31, false),
        (32, true),
        (127, true),
        (0x85, true),
        (0xd7ff, true),
        (0xd800, false),
        (0xdfff, false),
        (0xe000, true),
        (0xfffd, true),
        (0xfffe, false),
        (0xffff, false),
        (0x10000, true),
        (0x10ffff, true),
        (0x110000, false),
        (i64::MAX, false),
    ] {
        assert_eq!(valid_unicode_i64(value), expected, "{value:#x}");
    }
}

#[test]
fn source_noncharacter_acceptance_differs_from_a_general_noncharacter_filter() {
    for point in 0xfdd0..=0xfdef {
        assert!(valid_unicode_i64(point));
    }
    assert!(!valid_unicode_i64(0xfffe));
    assert!(!valid_unicode_i64(0xffff));
    for plane in 1..=16 {
        for tail in [0xfffe, 0xffff] {
            assert!(valid_unicode_i64(plane * 0x10000 + tail));
        }
    }
}

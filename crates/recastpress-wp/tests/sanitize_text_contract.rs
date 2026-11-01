use recastpress_wp::{
    SanitizeTextError as Error, SanitizeTextLimits,
    sanitize_text_field_utf8_site_no_open_angle_empty_filters_bytes as field,
    sanitize_textarea_field_utf8_site_no_open_angle_empty_filters_bytes as area,
};

fn limits() -> SanitizeTextLimits {
    SanitizeTextLimits::new(usize::MAX, usize::MAX, usize::MAX)
}

#[test]
fn field_and_textarea_differ_only_in_source_initial_cr_lf_tab_space_collapse() {
    let raw = b"\t a \r\n\t  b \t\n";
    assert_eq!(field(raw, limits()).unwrap(), b"a b");
    assert_eq!(area(raw, limits()).unwrap(), b"a \r\n\t  b");
    assert_eq!(field(b"a\x0bb\x0cc", limits()).unwrap(), b"a\x0bb\x0cc");
    assert_eq!(area(b"a\x0bb\x0cc", limits()).unwrap(), b"a\x0bb\x0cc");
}

#[test]
fn trim_six_removes_only_edge_nul_and_vertical_tab_and_retains_form_feed_nbsp() {
    for (raw, expected) in [
        (b"\0\x0b  a\0b\t\r\n\0".as_slice(), b"a\0b".as_slice()),
        (b"\x0c a \x0c", b"\x0c a \x0c"),
        (b"\xc2\xa0 a \xc2\xa0", b"\xc2\xa0 a \xc2\xa0"),
        (b"a\0\0b", b"a\0\0b"),
    ] {
        assert_eq!(field(raw, limits()).unwrap(), expected);
        assert_eq!(area(raw, limits()).unwrap(), expected);
    }
}

#[test]
fn percent_round_is_global_exact_case_not_match_count_or_case_folded_replacement() {
    assert_eq!(
        field(b"%41x%41", SanitizeTextLimits::new(7, 1, 1)).unwrap(),
        b"x"
    );
    assert_eq!(
        area(b"%41x%41", SanitizeTextLimits::new(7, 1, 1)).unwrap(),
        b"x"
    );
    let both = b"%Ab%aB";
    assert_eq!(
        field(both, SanitizeTextLimits::new(6, 0, 1)),
        Err(Error::PercentRoundLimit)
    );
    assert_eq!(
        area(both, SanitizeTextLimits::new(6, 0, 1)),
        Err(Error::PercentRoundLimit)
    );
    assert!(
        field(both, SanitizeTextLimits::new(6, 0, 2))
            .unwrap()
            .is_empty()
    );
    assert!(
        area(both, SanitizeTextLimits::new(6, 0, 2))
            .unwrap()
            .is_empty()
    );
    assert_eq!(field(b"%00%20%ff", limits()).unwrap(), b"");
    assert_eq!(area(b"not%2G%a%G1%", limits()).unwrap(), b"not%2G%a%G1%");
}

#[test]
fn deletion_cascades_are_seen_only_on_later_rounds_of_the_original_read_cursor() {
    for raw in [b"%a%41b".as_slice(), b"%%4141"] {
        let before = raw.to_vec();
        assert_eq!(
            field(raw, SanitizeTextLimits::new(raw.len(), 0, 1)),
            Err(Error::PercentRoundLimit)
        );
        assert_eq!(
            area(raw, SanitizeTextLimits::new(raw.len(), 0, 1)),
            Err(Error::PercentRoundLimit)
        );
        assert!(
            field(raw, SanitizeTextLimits::new(raw.len(), 0, 2))
                .unwrap()
                .is_empty()
        );
        assert!(
            area(raw, SanitizeTextLimits::new(raw.len(), 0, 2))
                .unwrap()
                .is_empty()
        );
        assert_eq!(raw, before);
    }
    // Budget zero is allowed if no token is present; exhausting exactly the
    // permitted rounds is success when the next find has no match.
    assert_eq!(
        field(b"%a", SanitizeTextLimits::new(2, 2, 0)).unwrap(),
        b"%a"
    );
    assert_eq!(
        area(b"%a", SanitizeTextLimits::new(2, 2, 0)).unwrap(),
        b"%a"
    );
    assert_eq!(
        field(b"%41", SanitizeTextLimits::new(3, 0, 0)),
        Err(Error::PercentRoundLimit)
    );
    assert!(
        field(b"%41", SanitizeTextLimits::new(3, 0, 1))
            .unwrap()
            .is_empty()
    );
    // Tight source-derived 4 KiB family: the initial %41 deletion joins one
    // rightmost %a with one b per later round. No newly joined token may be
    // removed during the same original-stream scan.
    let mut raw = b"%a".repeat(1364);
    raw.extend_from_slice(b"%41");
    raw.extend_from_slice(&[b'b'; 1364]);
    assert_eq!(raw.len(), 4095);
    let mut prefixed = Vec::with_capacity(4096);
    prefixed.push(b'x');
    prefixed.extend_from_slice(&raw);
    assert_eq!(prefixed.len(), 4096);
    for input in [&raw[..], &prefixed[..]] {
        let expected = if input.len() == 4095 {
            b"".as_slice()
        } else {
            b"x"
        };
        assert_eq!(
            field(input, SanitizeTextLimits::new(4096, 1, 1365)).unwrap(),
            expected
        );
        assert_eq!(
            area(input, SanitizeTextLimits::new(4096, 1, 1365)).unwrap(),
            expected
        );
        assert_eq!(
            field(input, SanitizeTextLimits::new(4096, 1, 1364)),
            Err(Error::PercentRoundLimit)
        );
        assert_eq!(
            area(input, SanitizeTextLimits::new(4096, 1, 1364)),
            Err(Error::PercentRoundLimit)
        );
    }
    assert!(
        field(&raw, SanitizeTextLimits::new(4096, 0, 1365))
            .unwrap()
            .is_empty()
    );
    assert!(
        area(&raw, SanitizeTextLimits::new(4096, 0, 1365))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        field(&prefixed, SanitizeTextLimits::new(4096, 0, 1365)),
        Err(Error::OutputLimit)
    );
    assert_eq!(
        area(&prefixed, SanitizeTextLimits::new(4096, 0, 1365)),
        Err(Error::OutputLimit)
    );
}

#[test]
fn final_ascii_space_collapse_occurs_only_if_percent_removal_happened() {
    assert_eq!(area(b"a  b", limits()).unwrap(), b"a  b");
    assert_eq!(field(b"a  b", limits()).unwrap(), b"a b");
    assert_eq!(area(b"a  %41 b", limits()).unwrap(), b"a b");
    assert_eq!(area(b"a\t%41\tb", limits()).unwrap(), b"a\t\tb");
    assert_eq!(field(b"a\t%41\tb", limits()).unwrap(), b"a b");
    assert_eq!(area(b"a\n%41\rb", limits()).unwrap(), b"a\n\rb");
    assert_eq!(area(b"%41\t\ntext\r%41", limits()).unwrap(), b"text");
}

#[test]
fn input_cap_is_first_and_final_output_cap_follows_all_shrinking_passes() {
    let raw = b"  a%41b  ";
    assert_eq!(
        field(raw, SanitizeTextLimits::new(raw.len(), 2, 1)).unwrap(),
        b"ab"
    );
    assert_eq!(
        area(raw, SanitizeTextLimits::new(raw.len(), 2, 1)).unwrap(),
        b"ab"
    );
    assert_eq!(
        field(raw, SanitizeTextLimits::new(raw.len() - 1, 2, 1)),
        Err(Error::InputLimit)
    );
    assert_eq!(
        area(raw, SanitizeTextLimits::new(raw.len(), 1, 1)),
        Err(Error::OutputLimit)
    );
    for raw in [b"".as_slice(), b" \t\r\n\x0b\0", b"%41%41"] {
        assert!(
            field(raw, SanitizeTextLimits::new(raw.len(), 0, 1))
                .unwrap()
                .is_empty()
        );
        assert!(
            area(raw, SanitizeTextLimits::new(raw.len(), 0, 1))
                .unwrap()
                .is_empty()
        );
    }
    // Round refusal precedes the final output-length check.
    assert_eq!(
        field(b"prefix%41", SanitizeTextLimits::new(9, 0, 0)),
        Err(Error::PercentRoundLimit)
    );
    assert_eq!(
        field(b"0", SanitizeTextLimits::new(1, 0, 0)),
        Err(Error::OutputLimit)
    );
}

#[test]
fn invalid_utf8_yields_empty_but_raw_open_angle_is_outside_the_profile_even_if_invalid() {
    for raw in [b"\xff".as_slice(), b"ok%41\xc0", b"trailing\xed\xa0\x80"] {
        assert!(
            field(raw, SanitizeTextLimits::new(raw.len(), 0, 0))
                .unwrap()
                .is_empty()
        );
        assert!(
            area(raw, SanitizeTextLimits::new(raw.len(), 0, 0))
                .unwrap()
                .is_empty()
        );
    }
    for raw in [b"<".as_slice(), b"\xff<", b"&lt; <tag>"] {
        assert_eq!(
            field(raw, SanitizeTextLimits::new(raw.len(), 0, 0)),
            Err(Error::UnsupportedValue)
        );
        assert_eq!(
            area(raw, SanitizeTextLimits::new(raw.len(), 0, 0)),
            Err(Error::UnsupportedValue)
        );
        assert_eq!(
            field(raw, SanitizeTextLimits::new(0, 0, 0)),
            Err(Error::InputLimit)
        );
    }
}

#[test]
fn valid_unicode_raw_entities_and_inputs_are_preserved_without_url_decoding_or_html_claims() {
    let raw = "  日本語 🦀 > &lt; &copy; %41a\0b  ".as_bytes();
    let before = raw.to_vec();
    let expected = "日本語 🦀 > &lt; &copy; a\0b".as_bytes();
    assert_eq!(field(raw, limits()).unwrap(), expected);
    assert_eq!(area(raw, limits()).unwrap(), expected);
    assert_eq!(raw, before);
    for error in [
        Error::UnsupportedValue,
        Error::InputLimit,
        Error::OutputLimit,
        Error::PercentRoundLimit,
        Error::AllocationFailed,
    ] {
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

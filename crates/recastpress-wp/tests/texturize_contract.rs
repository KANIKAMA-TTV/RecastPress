use recastpress_wp::{
    TexturizeError as Error, TexturizeLimits, wptexturize_default_english_no_html_bytes,
};

fn convert(raw: &[u8]) -> Vec<u8> {
    wptexturize_default_english_no_html_bytes(
        raw,
        TexturizeLimits::new(usize::MAX, usize::MAX, usize::MAX),
    )
    .unwrap()
}

#[test]
fn fixed_english_static_and_cockney_replacements_are_case_sensitive_and_ordered() {
    for (raw, expected) in [
        (b"... .... ..... ......".as_slice(), b"&#8230; &#8230;. &#8230;.. &#8230;&#8230;".as_slice()),
        (b"``word'' (tm)", b"&#8220;word&#8221; &#8482;"),
        (b"'tain't 'twere 'twas 'tis 'twill 'til 'bout 'nuff 'round 'cause 'em", b"&#8217;tain&#8217;t &#8217;twere &#8217;twas &#8217;tis &#8217;twill &#8217;til &#8217;bout &#8217;nuff &#8217;round &#8217;cause &#8217;em"),
        (b"x'tis (TM) (tm)", b"x&#8217;tis (TM) &#8482;"),
        (b"[code]...[/code]", b"[code]&#8230;[/code]"),
    ] {
        assert_eq!(convert(raw), expected);
    }
}

#[test]
fn apostrophe_year_and_decimal_quote_rules_run_before_prime_resolution() {
    for (raw, expected) in [
        (
            b"can't rock'n'roll".as_slice(),
            b"can&#8217;t rock&#8217;n&#8217;roll".as_slice(),
        ),
        (b"'99' '99\"", b"&#8217;99&#8217; &#8217;99&#8221;"),
        (b"'99 '99s '99's", b"&#8217;99 &#8217;99s &#8217;99&#8217;s"),
        (b"'0.42' \"42\"", b"&#8216;0.42&#8217; &#8220;42&#8221;"),
        (
            b"'9' '99%' '999' '99.0'",
            b"&#8216;9&#8217; &#8216;99%&#8217; &#8216;999&#8217; &#8216;99.0&#8217;",
        ),
        (b"9' 9\"", b"9&#8242; 9&#8243;"),
        (
            b"\"hello\" 'hello'",
            b"&#8220;hello&#8221; &#8216;hello&#8217;",
        ),
    ] {
        assert_eq!(convert(raw), expected);
    }
}

#[test]
fn source_whitespace_nbsp_entities_and_ascii_word_boundaries_remain_distinct() {
    for (raw, expected) in [
        (
            b"a\xc2\xa0'word'".as_slice(),
            b"a\xc2\xa0&#8216;word&#8217;".as_slice(),
        ),
        (b"a&nbsp;\"word\"", b"a&nbsp;&#8220;word&#8221;"),
        (b"a&NBSP;\"word\"", b"a&NBSP;&#8221;word&#8221;"),
        (b"\x0b'word'", b"\x0b&#8217;word&#8217;"),
        (b"\" word\"", b"&#8221; word&#8221;"),
        (b"&lt;'word'&gt;", b"&lt;&#8216;word&#8217;&gt;"),
        (
            "日本語 'はい' 🦀".as_bytes(),
            "日本語 &#8216;はい&#8217; 🦀".as_bytes(),
        ),
    ] {
        assert_eq!(convert(raw), expected);
    }
    for raw in [b"".as_slice(), b"0", b"\0\t\r\n \x0b", b"\x0c"] {
        assert_eq!(convert(raw), raw);
    }
}

#[test]
fn dashes_punycode_and_conditional_numeric_times_follow_the_ordered_ascii_patterns() {
    for (raw, expected) in [
        (
            b"--- -- - a--b a-b xn--abc XN--abc".as_slice(),
            b"&#8212; &#8212; &#8211; a&#8211;b a-b xn--abc XN&#8211;abc".as_slice(),
        ),
        (b"a\xc2\xa0--&nbsp;-\n", b"a\xc2\xa0&#8212;&nbsp;&#8211;\n"),
        (
            b"9x9 0x9999 00x9 0.x1 1x0",
            b"9&#215;9 0x9999 00&#215;9 0.&#215;1 1&#215;0",
        ),
        (b"a9x9 9x9a _9x9 9x9_ 9x9x9", b"a9x9 9x9a _9x9 9x9_ 9x9x9"),
        (
            "é9x9é 9x9. 1,2x3.4".as_bytes(),
            "é9&#215;9é 9&#215;9. 1,2&#215;3.4".as_bytes(),
        ),
    ] {
        assert_eq!(convert(raw), expected);
    }
}

#[test]
fn ampersand_guard_is_the_source_shallow_pattern_not_entity_name_validation() {
    for (raw, expected) in [
        (
            b"& &copy; &notreal; &abcdefgh; &abcdefghi;".as_slice(),
            b"&#038; &copy; &notreal; &abcdefgh; &#038;abcdefghi;".as_slice(),
        ),
        (
            b"&a1234; &a0; &a5; &1; &4; &5;",
            b"&a1234; &#038;a0; &#038;a5; &1; &4; &#038;5;",
        ),
        (
            b"&#0; &#999999999999999999; &#xAf; &#X00; &#x; &#-1;",
            b"&#0; &#999999999999999999; &#xAf; &#X00; &#038;#x; &#038;#-1;",
        ),
        (
            b"&amp;amp; &amp &; &AMP;",
            b"&amp;amp; &#038;amp &#038;; &AMP;",
        ),
    ] {
        assert_eq!(convert(raw), expected);
    }
}

#[test]
fn explicit_input_intermediate_and_final_caps_have_separate_failure_boundaries() {
    let raw = b"...";
    let expected = b"&#8230;";
    assert_eq!(
        wptexturize_default_english_no_html_bytes(raw, TexturizeLimits::new(3, 7, 7)),
        Ok(expected.to_vec())
    );
    for (limits, error) in [
        (TexturizeLimits::new(2, 7, 7), Error::InputLimit),
        (TexturizeLimits::new(3, 6, 7), Error::WorkLimit),
        (TexturizeLimits::new(3, 7, 6), Error::OutputLimit),
        (TexturizeLimits::new(3, 0, 0), Error::WorkLimit),
    ] {
        assert_eq!(
            wptexturize_default_english_no_html_bytes(raw, limits),
            Err(error)
        );
    }
    // Internal quote markers are longer than the final output and are charged.
    assert_eq!(
        wptexturize_default_english_no_html_bytes(b"'", TexturizeLimits::new(1, 7, 7)),
        Err(Error::WorkLimit)
    );
    assert_eq!(
        wptexturize_default_english_no_html_bytes(b"'", TexturizeLimits::new(1, 10, 7)),
        Ok(b"&#8216;".to_vec())
    );
    assert_eq!(
        wptexturize_default_english_no_html_bytes(b"", TexturizeLimits::new(0, 0, 0)),
        Ok(Vec::new())
    );
    assert_eq!(
        wptexturize_default_english_no_html_bytes(b"0", TexturizeLimits::new(1, 1, 0)),
        Err(Error::OutputLimit)
    );
}

#[test]
fn whole_utf8_no_literal_angle_admission_and_errors_keep_input_private_and_unchanged() {
    for raw in [
        b"PRIVATE <x>".as_slice(),
        b"prefix\xff",
        b"\"valid first\" trailing\xc0",
    ] {
        let before = raw.to_vec();
        assert_eq!(
            wptexturize_default_english_no_html_bytes(
                raw,
                TexturizeLimits::new(raw.len(), usize::MAX, usize::MAX)
            ),
            Err(Error::UnsupportedValue)
        );
        assert_eq!(raw, before);
    }
    let raw = b"PRIVATE\0 ... 'word'";
    let before = raw.to_vec();
    let output = convert(raw);
    assert_eq!(raw.as_slice(), before);
    assert_eq!(output, b"PRIVATE\0 &#8230; &#8216;word&#8217;");
    for error in [
        Error::UnsupportedValue,
        Error::InputLimit,
        Error::WorkLimit,
        Error::OutputLimit,
        Error::AllocationFailed,
    ] {
        let text = format!("{error:?} {error}");
        assert!(!text.contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

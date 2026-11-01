use recastpress_wp::{
    TitleDashContext::{Display, Save},
    TitleDashError, sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes as sanitize,
};
use std::error::Error;

#[test]
fn percent_protection_and_literal_sentinels_follow_two_nonrecursive_regexp_passes() {
    for (raw, expected) in [
        (b"%41%aF%z%".as_slice(), b"%41%afz".as_slice()),
        (b"---41---", b"%41"),
        (b"---C3------84---", b"%c3%84"),
        (b"----41---", b"%41"),
        (b"%4%30A", b"4%30a"),
        (b"%2541", b"%2541"),
        (b"%&x;41", b"41"),
        (b"---4x---", b"4x"),
    ] {
        assert_eq!(sanitize(raw, Display, usize::MAX).unwrap(), expected);
    }
}

#[test]
fn save_replacements_differ_from_display_and_do_not_remove_accents_or_lower_unicode() {
    for (raw, display, save) in [
        ("a\u{a0}b", "a%c2%a0b", "a-b"),
        ("a—b", "a%e2%80%94b", "a-b"),
        ("a\u{2009}b", "a%e2%80%89b", "a-b"),
        ("a\u{200b}b", "a%e2%80%8bb", "ab"),
        ("a©b", "a%c2%a9b", "ab"),
        ("a×b", "a%c3%97b", "axb"),
        ("a&NBSP;b", "ab", "a-b"),
        ("a&#8212;b", "ab", "a-b"),
        ("a/b", "ab", "a-b"),
        ("ÄÉ", "%c3%84%c3%89", "%c3%84%c3%89"),
        ("e\u{301}", "e%cc%81", "e"),
    ] {
        assert_eq!(
            sanitize(raw.as_bytes(), Display, usize::MAX).unwrap(),
            display.as_bytes()
        );
        assert_eq!(
            sanitize(raw.as_bytes(), Save, usize::MAX).unwrap(),
            save.as_bytes()
        );
    }
}

#[test]
fn entity_shortest_non_lf_pass_precedes_whitelist_and_only_space_can_become_a_dash() {
    for (raw, expected) in [
        (b"a&x;b".as_slice(), b"ab".as_slice()),
        (b"a&;b;c", b"ac"),
        (b"a&x\n;b", b"axb"),
        (b"a&x\r;b", b"ab"),
        (b"a\tb\nc\rd\x0be\x0cf", b"abcdef"),
        (b"a b", b"a-b"),
        (b" --A... - B__-- ", b"a-b__"),
        (b"\0 A\0B \0", b"ab"),
        (b"a>+@!b", b"ab"),
    ] {
        assert_eq!(sanitize(raw, Display, usize::MAX).unwrap(), expected);
    }
}

#[test]
fn no_open_angle_and_post_nul_utf8_admission_are_explicit_without_latin1_fallback() {
    assert_eq!(sanitize(b"\xc3\0\x84", Display, 6), Ok(b"%c3%84".to_vec()));
    for raw in [
        b"<".as_slice(),
        b"SECRET<private>",
        b"\xff",
        b"\xc3",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
    ] {
        assert_eq!(
            sanitize(raw, Save, 0),
            Err(TitleDashError::UnsupportedValue)
        );
    }
    let saved = b"\0\xc3\x84 SECRET\0".to_vec();
    let original = saved.clone();
    sanitize(&saved, Save, usize::MAX).unwrap();
    assert_eq!(saved, original);
}

#[test]
fn fixed_200_byte_uri_limit_runs_before_cleanup_without_initial_trim_or_octet_safety() {
    let mut leading_spaces = vec![b' '; 200];
    leading_spaces.push(b'A');
    assert_eq!(sanitize(&leading_spaces, Display, 0), Ok(Vec::new()));
    assert_eq!(sanitize(&b"&a;".repeat(67), Display, 1), Ok(b"a".to_vec()));
    let mut raw = vec![b'a'; 199];
    raw.extend_from_slice(b"%41");
    let mut expected = vec![b'a'; 199];
    expected.push(b'%');
    assert_eq!(sanitize(&raw, Display, 200), Ok(expected));
    assert_eq!(
        sanitize("界".repeat(23).as_bytes(), Display, 198),
        Ok(b"%e7%95%8c".repeat(22))
    );
    assert_eq!(
        sanitize("𐍈".repeat(17).as_bytes(), Display, 192),
        Ok(b"%f0%90%8d%88".repeat(16))
    );
    let mut removed = b"%c2%ad".repeat(33); // 198 encoded ASCII bytes before save cleanup.
    removed.extend_from_slice(b"ABC");
    assert_eq!(sanitize(&removed, Save, 2), Ok(b"ab".to_vec()));
}

#[test]
fn only_final_bytes_are_capped_and_errors_never_retain_private_input() {
    assert_eq!(
        sanitize(&vec![b'?'; 1024 * 1024], Display, 0),
        Ok(Vec::new())
    );
    for (raw, context, expected) in [
        (b"---".as_slice(), Display, b"".as_slice()),
        (b"%", Display, b""),
        (b"%c2%ad", Save, b""),
        (b"a&entity;b", Display, b"ab"),
        (b"a%c2%a0b", Save, b"a-b"),
        ("日本".as_bytes(), Display, b"%e6%97%a5%e6%9c%ac"),
    ] {
        assert_eq!(
            sanitize(raw, context, expected.len()),
            Ok(expected.to_vec())
        );
        if !expected.is_empty() {
            assert_eq!(
                sanitize(raw, context, expected.len() - 1),
                Err(TitleDashError::OutputLimit)
            );
        }
    }
    let error = sanitize(b"SECRET_PRIVATE<", Save, 0).unwrap_err();
    assert_eq!(format!("{error:?}"), "UnsupportedValue");
    assert!(!error.to_string().contains("SECRET"));
    assert!(error.source().is_none());
}

use recastpress_wp::{
    SanitizeTitleContext::{Display, Query, Save},
    SanitizeTitleError, sanitize_title_utf8_default_callback_explicit_locale_bytes as sanitize,
};
use std::error::Error;

#[test]
fn save_removes_accents_with_explicit_locale_before_the_default_callback() {
    for (locale, expected) in [
        (b"en_US".as_slice(), b"a".as_slice()),
        (b"de_DE", b"ae"),
        (b"default", b"ae"),
    ] {
        assert_eq!(
            sanitize("Ä".as_bytes(), b"fallback", Save, locale, 2).unwrap(),
            expected
        );
        for context in [Display, Query] {
            assert_eq!(
                sanitize("Ä".as_bytes(), b"fallback", context, locale, 6).unwrap(),
                b"%c3%84"
            );
        }
    }
    assert_eq!(
        sanitize("l·l".as_bytes(), b"fallback", Save, b"ca", 2).unwrap(),
        b"ll"
    );
    assert_eq!(
        sanitize("Đ".as_bytes(), b"fallback", Save, b"bs_BA", 2).unwrap(),
        b"dj"
    );
}

#[test]
fn accent_deletion_precedes_the_200_byte_uri_prefix_in_save_only() {
    let raw = format!("{}A", "£".repeat(201));
    assert_eq!(
        sanitize(raw.as_bytes(), b"fallback", Save, b"en_US", 1).unwrap(),
        b"a"
    );
    for context in [Display, Query] {
        assert_eq!(
            sanitize(raw.as_bytes(), b"fallback", context, b"en_US", 198).unwrap(),
            b"%c2%a3".repeat(33)
        );
    }
    let raw = format!("{}A", "Ä".repeat(200));
    assert_eq!(
        sanitize(raw.as_bytes(), b"fallback", Save, b"en_US", 200).unwrap(),
        vec![b'a'; 200]
    );
}

#[test]
fn only_empty_callback_output_selects_unmodified_arbitrary_fallback_bytes() {
    let fallback = b"\xff\0<PRIVATE>& %41";
    for context in [Save, Display, Query] {
        for raw in [b"".as_slice(), b"???", b"---"] {
            assert_eq!(
                sanitize(raw, fallback, context, b"en_US", fallback.len()).unwrap(),
                fallback
            );
        }
        assert_eq!(
            sanitize(b"0", fallback, context, b"en_US", 1).unwrap(),
            b"0"
        );
        assert_eq!(sanitize(b"?", b"0", context, b"en_US", 1).unwrap(), b"0");
        assert_eq!(sanitize(b"?", b"", context, b"en_US", 0).unwrap(), b"");
    }
}

#[test]
fn raw_whole_utf8_no_angle_and_truthy_locale_are_uniform_admission_guards() {
    for context in [Save, Display, Query] {
        for raw in [b"\xc3\0\x84".as_slice(), b"<", b"A\xff"] {
            assert_eq!(
                sanitize(raw, b"fallback", context, b"en_US", usize::MAX),
                Err(SanitizeTitleError::UnsupportedValue)
            );
        }
        for locale in [b"".as_slice(), b"0"] {
            assert_eq!(
                sanitize(b"ASCII", b"fallback", context, locale, 0),
                Err(SanitizeTitleError::UnsupportedValue)
            );
        }
        assert_eq!(
            sanitize(b"A\0B", b"fallback", context, b"\xfflocale", 2).unwrap(),
            b"ab"
        );
    }
    let mut raw = vec![b'A'; 201];
    raw.push(0xff);
    assert_eq!(
        sanitize(&raw, b"fallback", Display, b"en_US", 200),
        Err(SanitizeTitleError::UnsupportedValue)
    );
}

#[test]
fn final_cap_applies_after_cleanup_and_fallback_selection_without_error_fallback() {
    assert_eq!(
        sanitize(&vec![b'?'; 4096], b"", Save, b"en_US", 0).unwrap(),
        b""
    );
    assert_eq!(
        sanitize(
            b"a&entity;b",
            b"unused oversized fallback",
            Save,
            b"en_US",
            2
        )
        .unwrap(),
        b"ab"
    );
    assert_eq!(
        sanitize(b"a&entity;b", b"", Save, b"en_US", 1),
        Err(SanitizeTitleError::OutputLimit)
    );
    assert_eq!(
        sanitize(b"?", b"fallback", Query, b"en_US", 7),
        Err(SanitizeTitleError::OutputLimit)
    );
    assert_eq!(
        sanitize(b"?", b"fallback", Query, b"en_US", 8).unwrap(),
        b"fallback"
    );
    assert_eq!(
        sanitize(b"a", b"", Display, b"en_US", 0),
        Err(SanitizeTitleError::OutputLimit)
    );
}

#[test]
fn source_value_semantics_preserve_inputs_and_closed_errors_reveal_no_payload() {
    let raw = "Ä &nbsp; A".as_bytes().to_vec();
    let fallback = b"PRIVATE_FALLBACK\xff".to_vec();
    let locale = b"de_DE".to_vec();
    let originals = (raw.clone(), fallback.clone(), locale.clone());
    assert_eq!(
        sanitize(&raw, &fallback, Save, &locale, usize::MAX).unwrap(),
        b"ae-a"
    );
    assert_eq!((raw, fallback, locale), originals);
    for error in [
        SanitizeTitleError::UnsupportedValue,
        SanitizeTitleError::OutputLimit,
        SanitizeTitleError::AllocationFailed,
    ] {
        assert!(error.source().is_none());
        assert!(!format!("{error:?}: {error}").contains("PRIVATE"));
    }
}

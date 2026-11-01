use recastpress_wp::{RemoveAccentsError, remove_accents_utf8_explicit_locale_bytes as remove};

#[test]
fn locale_specific_literal_mappings_are_exact_and_german_matches_every_lowercase_de_prefix() {
    let input = "ÄäÖöÜüẞß ÆæØøÅå Đđ l·l €£";
    assert_eq!(
        remove(input.as_bytes(), b"en_US", usize::MAX).unwrap(),
        "AaOoUuẞs AEaeOoAa Dd l·l E".as_bytes()
    );
    for locale in [
        b"de".as_slice(),
        b"de_DE",
        b"de_CH_informal",
        b"de_AT",
        b"default",
        b"de\xff\0",
    ] {
        assert_eq!(
            remove("ÄäÖöÜüẞß".as_bytes(), locale, usize::MAX).unwrap(),
            b"AeaeOeoeUeueSSss"
        );
    }
    assert_eq!(
        remove("ÆæØøÅå".as_bytes(), b"da_DK", usize::MAX).unwrap(),
        b"AeaeOeoeAaaa"
    );
    for locale in [b"sr_RS".as_slice(), b"bs_BA"] {
        assert_eq!(
            remove("Đđ".as_bytes(), locale, usize::MAX).unwrap(),
            b"DJdj"
        );
    }
    for locale in [
        b"DE".as_slice(),
        b" de",
        b"ca\0",
        b"da_DK\0",
        b"sr_RS_suffix",
        b"\xff",
        b"0\0",
    ] {
        assert_eq!(
            remove("ÄÆĐl·l".as_bytes(), locale, usize::MAX).unwrap(),
            "AAEDl·l".as_bytes()
        );
    }
}

#[test]
fn catalan_uses_longest_original_key_without_recursion_or_unicode_normalization() {
    let input = "l·l·l ll·l L·L l·L é e\u{301} ẞ \u{feff}\u{fdd0} 日本語🦀\0";
    let saved = input.as_bytes().to_vec();
    assert_eq!(
        remove(input.as_bytes(), b"ca", usize::MAX).unwrap(),
        "ll·l lll L·L l·L e e\u{301} ẞ \u{feff}\u{fdd0} 日本語🦀\0".as_bytes()
    );
    assert_eq!(input.as_bytes(), saved);
    assert_eq!(
        remove("l·l·l".as_bytes(), b"en_US", usize::MAX).unwrap(),
        "l·l·l".as_bytes()
    );
}

#[test]
fn only_final_output_bytes_are_capped_with_deletion_expansion_and_empty_zero_boundaries() {
    assert_eq!(remove("£££££".as_bytes(), b"en_US", 0), Ok(Vec::new()));
    assert_eq!(remove(b"", b"ca", 0), Ok(Vec::new()));
    assert_eq!(remove("£Ä".as_bytes(), b"de", 2), Ok(b"Ae".to_vec()));
    assert_eq!(
        remove("£Ä".as_bytes(), b"de", 1),
        Err(RemoveAccentsError::OutputLimit)
    );
    assert_eq!(
        remove(b"plain\0", b"en_US", 5),
        Err(RemoveAccentsError::OutputLimit)
    );
    assert_eq!(remove(b"plain\0", b"en_US", 6), Ok(b"plain\0".to_vec()));
    let saved = "ÆœÞ".as_bytes().to_vec();
    assert_eq!(remove(&saved, b"en_US", 6), Ok(b"AEoeTH".to_vec()));
    assert_eq!(saved, "ÆœÞ".as_bytes());
}

#[test]
fn invalid_utf8_and_php_empty_locale_domain_are_rejected_before_output_limits() {
    for invalid in [
        b"\xff".as_slice(),
        b"\xc0\xaf",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"ok\xe2\x82",
    ] {
        assert_eq!(
            remove(invalid, b"en_US", 0),
            Err(RemoveAccentsError::UnsupportedValue)
        );
    }
    for locale in [b"".as_slice(), b"0"] {
        assert_eq!(
            remove(b"ASCII", locale, 0),
            Err(RemoveAccentsError::UnsupportedValue)
        );
        assert_eq!(
            remove(b"", locale, 0),
            Err(RemoveAccentsError::UnsupportedValue)
        );
    }
}

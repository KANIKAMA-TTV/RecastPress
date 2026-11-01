use recastpress_wp::{
    SpecialCharsError, SpecialCharsQuoteStyle, esc_attr_utf8_empty_filters_bytes as attr,
    esc_html_utf8_empty_filters_bytes as html, wp_specialchars_utf8_bytes,
};

type Escape = fn(&[u8], usize) -> Result<Vec<u8>, SpecialCharsError>;
const ESCAPERS: [Escape; 2] = [html, attr];

#[test]
fn whole_input_invalid_utf8_becomes_empty_before_the_special_character_guard_or_cap() {
    for escape in ESCAPERS {
        for invalid in [
            b"\xff".as_slice(),
            b"\xc0\x80",
            b"\xed\xa0\x80",
            b"\xf4\x90\x80\x80",
            b"\xe2\x82",
        ] {
            assert_eq!(escape(invalid, 0).unwrap(), b"");
            for special in b"&<>\"'" {
                let mut bytes = vec![*special];
                bytes.extend_from_slice(invalid);
                bytes.push(*special);
                assert_eq!(escape(&bytes, 0).unwrap(), b"");
            }
        }
    }
    assert_eq!(
        wp_specialchars_utf8_bytes(b"\xff", SpecialCharsQuoteStyle::Quotes, false, 1).unwrap(),
        b"\xff"
    );
}

#[test]
fn quotes_html401_and_numeric_normalization_share_the_empty_filter_byte_result() {
    let input = b"&<>\"'&copy;&apos;&#00039;&#X00027;&amp;copy;&#38;copy;";
    let expected = b"&amp;&lt;&gt;&quot;&#039;&copy;&amp;apos;&#039;&#x27;&amp;copy;&#038;copy;";
    for escape in ESCAPERS {
        assert_eq!(escape(input, expected.len()).unwrap(), expected);
    }
}

#[test]
fn final_cap_applies_after_normalizer_shrink_with_empty_exact_minus_one_and_max_boundaries() {
    let mut input = b"&#".to_vec();
    input.extend(std::iter::repeat_n(b'0', 4096));
    input.extend_from_slice(b"39;");
    for escape in ESCAPERS {
        assert_eq!(escape(b"", 0).unwrap(), b"");
        assert_eq!(escape(&input, 6).unwrap(), b"&#039;");
        assert_eq!(escape(&input, 5), Err(SpecialCharsError::OutputLimit));
        assert_eq!(escape(b"&", 0), Err(SpecialCharsError::OutputLimit));
        assert_eq!(escape(b"&", 5).unwrap(), b"&amp;");
        assert_eq!(escape(b"&", 4), Err(SpecialCharsError::OutputLimit));
        assert_eq!(escape(b"&", usize::MAX).unwrap(), b"&amp;");
    }
}

#[test]
fn valid_unicode_controls_and_noncharacters_are_preserved_without_input_mutation() {
    let input = "\0\u{1}\u{7f}\u{fdd0}\u{fffe}\u{10ffff}日本語😀&".as_bytes();
    let saved = input.to_vec();
    let mut expected = input[..input.len() - 1].to_vec();
    expected.extend_from_slice(b"&amp;");
    for escape in ESCAPERS {
        assert_eq!(escape(input, expected.len()).unwrap(), expected);
        for byte in 0..=u8::MAX {
            let value = [byte];
            assert_eq!(
                escape(&value, usize::MAX).unwrap(),
                if byte >= 128 {
                    vec![]
                } else {
                    wp_specialchars_utf8_bytes(
                        &value,
                        SpecialCharsQuoteStyle::Quotes,
                        false,
                        usize::MAX,
                    )
                    .unwrap()
                }
            );
        }
    }
    assert_eq!(input, saved);
}

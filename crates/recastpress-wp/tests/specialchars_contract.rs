use recastpress_wp::{
    SpecialCharsError, SpecialCharsQuoteStyle as Style, wp_specialchars_utf8_bytes as encode,
};

const STYLES: [Style; 6] = [
    Style::NoQuotes,
    Style::Compat,
    Style::Quotes,
    Style::Single,
    Style::Double,
    Style::Xml1,
];

#[test]
fn selected_quote_styles_escape_only_their_selected_quotes_and_share_double_encoding() {
    let expectations = [
        (Style::NoQuotes, b"&amp;&lt;&gt;\"'".as_slice()),
        (Style::Compat, b"&amp;&lt;&gt;&quot;'".as_slice()),
        (Style::Quotes, b"&amp;&lt;&gt;&quot;&#039;".as_slice()),
        (Style::Single, b"&amp;&lt;&gt;\"&#039;".as_slice()),
        (Style::Double, b"&amp;&lt;&gt;&quot;'".as_slice()),
        (Style::Xml1, b"&amp;&lt;&gt;&quot;&apos;".as_slice()),
    ];
    for (style, expected) in expectations {
        for double in [false, true] {
            assert_eq!(encode(b"&<>\"'", style, double, 99).unwrap(), expected);
        }
    }
}

#[test]
fn original_special_guard_controls_invalid_utf8_before_any_normalization_or_final_cap() {
    for style in STYLES {
        for double in [false, true] {
            for invalid in [
                b"\xff".as_slice(),
                b"\xc0\x80",
                b"\xed\xa0\x80",
                b"\xf4\x90\x80\x80",
                b"\xe2\x82",
            ] {
                assert_eq!(
                    encode(invalid, style, double, invalid.len()).unwrap(),
                    invalid
                );
                assert_eq!(
                    encode(invalid, style, double, 0),
                    Err(SpecialCharsError::OutputLimit)
                );
                for special in b"&<>\"'" {
                    let mut input = invalid.to_vec();
                    input.push(*special);
                    assert_eq!(encode(&input, style, double, 0).unwrap(), b"");
                }
            }
        }
    }
}

#[test]
fn html401_excludes_apos_while_xml_preserves_xml5_and_converts_other_names() {
    let input = b"&copy;&euro;&apos;&quot;&amp;&lt;&gt;&fjlig;";
    assert_eq!(
        encode(input, Style::Quotes, false, 99).unwrap(),
        b"&copy;&euro;&amp;apos;&quot;&amp;&lt;&gt;&amp;fjlig;"
    );
    assert_eq!(
        encode(input, Style::Xml1, false, 99).unwrap(),
        "©€&apos;&quot;&amp;&lt;&gt;&amp;fjlig;".as_bytes()
    );
    for style in STYLES {
        assert_eq!(
            encode(input, style, true, 999).unwrap(),
            b"&amp;copy;&amp;euro;&amp;apos;&amp;quot;&amp;amp;&amp;lt;&amp;gt;&amp;fjlig;"
        );
    }
}

#[test]
fn numeric_references_and_generated_ampersands_preserve_source_order_without_recursive_decode() {
    for style in STYLES {
        assert_eq!(
            encode(
                b"&#00039;&#X00027;&#00034;&amp;copy;&#38;copy;&#x26;copy;",
                style,
                false,
                999
            )
            .unwrap(),
            b"&#039;&#x27;&#034;&amp;copy;&#038;copy;&#x26;copy;"
        );
        assert_eq!(
            encode(
                b"&#0;&#X000D800;&#65534;&#x1FFFE;&#x10FFFF;",
                style,
                false,
                999
            )
            .unwrap(),
            b"&amp;#0;&amp;#x000D800;&amp;#65534;&#x1FFFE;&#x10FFFF;"
        );
    }
    assert_eq!(
        encode(b"'\"&#039;&amp;apos;", Style::Single, false, 99).unwrap(),
        b"&#039;\"&#039;&amp;apos;"
    );
}

#[test]
fn final_cap_is_exact_after_normalizer_shrink_and_never_caps_intermediate_bytes() {
    let mut input = b"&#".to_vec();
    input.extend(std::iter::repeat_n(b'0', 4096));
    input.extend_from_slice(b"39;");
    for style in STYLES {
        assert_eq!(encode(&input, style, false, 6).unwrap(), b"&#039;");
        assert_eq!(
            encode(&input, style, false, 5),
            Err(SpecialCharsError::OutputLimit)
        );
        for double in [false, true] {
            let expected = encode(b"&copy;'\"<>&unknown;", style, double, usize::MAX).unwrap();
            assert_eq!(
                encode(b"&copy;'\"<>&unknown;", style, double, expected.len()).unwrap(),
                expected
            );
            assert_eq!(
                encode(b"&copy;'\"<>&unknown;", style, double, expected.len() - 1),
                Err(SpecialCharsError::OutputLimit)
            );
            assert_eq!(encode(b"", style, double, 0).unwrap(), b"");
        }
    }
}

#[test]
fn valid_nul_controls_noncharacters_and_unicode_are_not_scrubbed_or_mutated() {
    let input = "\0\u{1}\u{7f}\u{fdd0}\u{fffe}\u{10ffff}日本語😀&".as_bytes();
    let saved = input.to_vec();
    let mut expected = input[..input.len() - 1].to_vec();
    expected.extend_from_slice(b"&amp;");
    for style in STYLES {
        for double in [false, true] {
            assert_eq!(encode(input, style, double, 999).unwrap(), expected);
        }
    }
    assert_eq!(input, saved);
}

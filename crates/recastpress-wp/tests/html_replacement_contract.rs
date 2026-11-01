use recastpress_wp::{
    CompatibilityError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_replace_in_html_tags_bytes,
};

fn pairs(entries: &[(&[u8], &[u8])]) -> PhpArray {
    let mut result = PhpArray::new();
    for (key, value) in entries {
        result.insert(PhpKey::Bytes(key.to_vec()), PhpValue::Bytes(value.to_vec()));
    }
    result
}

#[test]
fn html_replacement_single_is_nonoverlapping_and_does_not_rescan_output() {
    let replacements = pairs(&[(b"aa", b"aaa")]);
    let before = replacements.clone();
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"aa<aaaaa>aa", &replacements).unwrap(),
        b"aa<aaaaaaa>aa"
    );
    assert_eq!(replacements, before);
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"<a>a", &pairs(&[(b"a", b"<b>")])).unwrap(),
        b"<<b>>a"
    );
}

#[test]
fn html_replacement_multi_prefers_longest_independent_of_insertion_order() {
    for replacements in [
        pairs(&[(b"a", b"X"), (b"ab", b"Y")]),
        pairs(&[(b"ab", b"Y"), (b"a", b"X")]),
    ] {
        assert_eq!(
            wp_replace_in_html_tags_bytes(b"<ababa>ababa", &replacements).unwrap(),
            b"<YYX>ababa"
        );
    }
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"<aaaa>", &pairs(&[(b"aa", b"X"), (b"aaa", b"Y")])).unwrap(),
        b"<Ya>"
    );
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"<a>ab", &pairs(&[(b"a", b"b"), (b"b", b"c")])).unwrap(),
        b"<b>ab"
    );
}

#[test]
fn html_replacement_empty_map_single_empty_key_and_empty_values_are_explicit() {
    for replacements in [PhpArray::new(), pairs(&[(b"", b"replacement")])] {
        assert_eq!(
            wp_replace_in_html_tags_bytes(b"plain<a>\0\xff", &replacements).unwrap(),
            b"plain<a>\0\xff"
        );
    }
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"<abab>ab", &pairs(&[(b"ab", b""), (b"a", b"X")])).unwrap(),
        b"<>ab"
    );
}

#[test]
fn html_replacement_rejects_the_whole_unsupported_map_even_without_a_hit() {
    let mut unsupported = vec![
        pairs(&[(b"", b"x"), (b"absent", b"y")]),
        pairs(&[(b"0", b"x")]),
        pairs(&[(b"-2", b"x")]),
    ];
    for value in [
        PhpValue::Null,
        PhpValue::Bool(false),
        PhpValue::Integer(1),
        PhpValue::Float(1.0),
        PhpValue::Array(PhpArray::new()),
        PhpValue::Unsupported,
    ] {
        let mut replacements = pairs(&[(b"a", b"valid")]);
        replacements.insert(PhpKey::Bytes(b"absent".to_vec()), value);
        unsupported.push(replacements);
    }
    for replacements in unsupported {
        let before = replacements.clone();
        for input in [b"plain".as_slice(), b"<a>"] {
            assert_eq!(
                wp_replace_in_html_tags_bytes(input, &replacements),
                Err(CompatibilityError::UnsupportedValue)
            );
        }
        assert_eq!(replacements, before);
    }
}

#[test]
fn html_replacement_follows_byte_split_boundaries_including_malformed_markup() {
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"<a q='>'>a", &pairs(&[(b"a", b"X")])).unwrap(),
        b"<X q='>'>a"
    );
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"a<!--a>z<![CDATA[a]]>a", &pairs(&[(b"a", b"X")])).unwrap(),
        b"a<!--X>z<![CDATA[X]]>X"
    );
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"\xff<\xff\0>\xff", &pairs(&[(b"\xff\0", b"\0\xfe")]))
            .unwrap(),
        b"\xff<\0\xfe>\xff"
    );
    assert_eq!(
        wp_replace_in_html_tags_bytes(b"01<01>01", &pairs(&[(b"01", b"yes")])).unwrap(),
        b"01<yes>01"
    );
}

use recastpress_wp::{
    EntityNormalizeError, wp_kses_normalize_entities_default_html_bytes as html,
    wp_kses_normalize_entities_default_xml_utf8_bytes as xml,
};

#[path = "../src/xml_entity_mapping.rs"]
mod mapping;

#[test]
fn observed_table_preserves_xml5_and_converts_all_other_fixed_names() {
    assert_eq!(mapping::SORTED_XML.len(), 253);
    for (name, expected) in mapping::SORTED_XML {
        let mut input = vec![b'&'];
        input.extend_from_slice(name);
        input.push(b';');
        assert_eq!(xml(&input, usize::MAX).unwrap(), expected);
        assert_eq!(html(&input, usize::MAX).unwrap(), input);
        assert!(std::str::from_utf8(expected).is_ok());
    }
    assert_eq!(
        xml(b"&amp;&lt;&gt;&apos;&quot;", 99).unwrap(),
        b"&amp;&lt;&gt;&apos;&quot;"
    );
    assert_eq!(xml(b"&euro;&copy;&Alpha;", 99).unwrap(), "€©Α".as_bytes());
}

#[test]
fn numeric_canonicalization_and_original_ampersands_are_never_recursively_decoded() {
    assert_eq!(
        xml(b"&amp;copy; &#38;copy; &#x26;copy; &amp;#x26;copy;", 99).unwrap(),
        b"&amp;copy; &#038;copy; &#x26;copy; &amp;#x26;copy;"
    );
    assert_eq!(
        xml(b"&#0009;&#X000a;&#X000D800;&#X0000;&#0;", 99).unwrap(),
        b"&#009;&#xa;&amp;#x000D800;&amp;#X0000;&amp;#0;"
    );
}

#[test]
fn unknown_names_semicolons_case_and_raw_binary_preserve_the_byte_domain() {
    let input = b"\0\xff&fjlig;&NotEqualTilde;&COPY;&copy &copy;;&copy;";
    let before = input.to_vec();
    assert_eq!(
        xml(input, 999).unwrap(),
        b"\0\xff&amp;fjlig;&amp;NotEqualTilde;&amp;COPY;&amp;copy \xc2\xa9;\xc2\xa9"
    );
    assert_eq!(input.as_slice(), before);
}

#[test]
fn exact_final_cap_covers_shrinking_mapping_and_numeric_and_expanding_unknowns() {
    for input in [
        b"&copy;".as_slice(),
        b"&euro;&unknown;&#0000000000009;",
        b"\xff&",
    ] {
        let expected = xml(input, usize::MAX).unwrap();
        assert_eq!(xml(input, expected.len()).unwrap(), expected);
        assert_eq!(
            xml(input, expected.len() - 1),
            Err(EntityNormalizeError::OutputLimit)
        );
        assert_eq!(xml(input, 0), Err(EntityNormalizeError::OutputLimit));
    }
    assert_eq!(xml(b"", 0).unwrap(), b"");
}

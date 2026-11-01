use recastpress_wp::{
    EntityNormalizeError, wp_kses_normalize_entities_default_html_bytes as normalize,
};

#[test]
fn named_entities_use_case_sensitive_fixed_membership_without_recursive_decoding() {
    assert_eq!(
        normalize(b"AT&T &amp; &AMP; &apos; &copy; &notinva;", 128).unwrap(),
        b"AT&amp;T &amp; &amp;AMP; &apos; &copy; &amp;notinva;"
    );
    assert_eq!(
        normalize(b"&amp;#x2E; &#x2E; &amp;amp; &&copy;", 128).unwrap(),
        b"&amp;#x2E; &#x2E; &amp;amp; &amp;&copy;"
    );
}

#[test]
fn numeric_matches_preserve_callback_order_case_and_unmatched_forms() {
    assert_eq!(
        normalize(b"&#9; &#00065; &#X000D; &#x1fffe; &#0; &#X0000;", 128).unwrap(),
        b"&#009; &#065; &#xD; &#x1fffe; &amp;#0; &amp;#X0000;"
    );
    assert_eq!(
        normalize(b"&#0001; &#X00D800; &#xFFFF; &#x110000; &#99999999;", 128).unwrap(),
        b"&amp;#0001; &amp;#x00D800; &amp;#xFFFF; &amp;#x110000; &amp;#99999999;"
    );
    assert_eq!(
        normalize(b"&#65 &#x0A &copy\n;", 64).unwrap(),
        b"&amp;#65 &amp;#x0A &amp;copy\n;"
    );
}

#[test]
fn final_cap_counts_escaping_and_leading_zero_compression_exactly() {
    assert_eq!(normalize(b"", 0), Ok(Vec::new()));
    assert_eq!(normalize(b"&", 5), Ok(b"&amp;".to_vec()));
    assert_eq!(normalize(b"&", 4), Err(EntityNormalizeError::OutputLimit));
    let mut padded = b"&#".to_vec();
    padded.extend_from_slice(&vec![b'0'; 16384]);
    padded.extend_from_slice(b"65;");
    assert_eq!(normalize(&padded, 6), Ok(b"&#065;".to_vec()));
    assert_eq!(
        normalize(&padded, 5),
        Err(EntityNormalizeError::OutputLimit)
    );
    let input = b"a\0\xff&copy;&&";
    let expected = b"a\0\xff&copy;&amp;&amp;";
    assert_eq!(normalize(input, expected.len()), Ok(expected.to_vec()));
    assert_eq!(
        normalize(input, expected.len() - 1),
        Err(EntityNormalizeError::OutputLimit)
    );
    assert_eq!(input, b"a\0\xff&copy;&&");
}

#[test]
fn numeric_character_boundaries_keep_the_fixed_unicode_predicate() {
    assert_eq!(
        normalize(b"&#8;|&#9;|&#10;|&#11;|&#13;|&#31;|&#32;", 128).unwrap(),
        b"&amp;#8;|&#009;|&#010;|&amp;#11;|&#013;|&amp;#31;|&#032;"
    );
    assert_eq!(
        normalize(b"&#xD7FF;|&#xD800;|&#xE000;|&#xFDD0;|&#x10FFFF;", 128).unwrap(),
        b"&#xD7FF;|&amp;#xD800;|&#xE000;|&#xFDD0;|&#x10FFFF;"
    );
}

#[test]
fn checked_default_table_and_all_source_names_remain_borrowed_membership_data() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../compatibility/wordpress-default-html-entities.json"
    ))
    .unwrap();
    let names = fixture["names"].as_array().unwrap();
    assert_eq!(names.len(), 253);
    for name in names {
        let input = format!("&{};", name.as_str().unwrap()).into_bytes();
        assert_eq!(normalize(&input, input.len()), Ok(input));
    }
}

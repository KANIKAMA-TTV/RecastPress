use recastpress_wp::{wp_is_json_request_c_bytes, wp_is_xml_request_bytes};

#[test]
fn absent_and_present_empty_headers_produce_no_hint() {
    for accept in [None, Some(b"".as_slice())] {
        for content_type in [None, Some(b"".as_slice())] {
            assert!(!wp_is_json_request_c_bytes(accept, content_type));
            assert!(!wp_is_xml_request_bytes(accept, content_type));
        }
    }
    assert!(wp_is_json_request_c_bytes(None, Some(b"application/json")));
    assert!(wp_is_xml_request_bytes(None, Some(b"text/xml")));
}

#[test]
fn json_uses_accept_then_content_type_with_the_source_media_boundaries() {
    for (accept, content_type, expected) in [
        (b"application/json".as_slice(), b"not-json".as_slice(), true),
        (b"not-json", b"APPLICATION/JSON+OEMBED", true),
        (b"application/jsonx", b"application/+json", false),
        (b"text/plain,application/problem+json;q=0", b"", true),
        (b"", b"application/json; charset=utf8", true),
        (b"application/x,application/+json", b"", true),
        (b"xapplication/json", b"application/json\0", false),
    ] {
        assert_eq!(
            wp_is_json_request_c_bytes(Some(accept), Some(content_type)),
            expected,
            "accept={accept:?}, content_type={content_type:?}"
        );
    }
}

#[test]
fn xml_accept_substrings_and_content_type_exact_membership_are_distinct() {
    for kind in [
        b"text/xml".as_slice(),
        b"application/rss+xml",
        b"application/atom+xml",
        b"application/rdf+xml",
        b"text/xml+oembed",
        b"application/xml+oembed",
    ] {
        assert!(wp_is_xml_request_bytes(Some(kind), None));
        assert!(wp_is_xml_request_bytes(None, Some(kind)));
        let mut parameterized = kind.to_vec();
        parameterized.extend_from_slice(b"; charset=utf8");
        assert!(wp_is_xml_request_bytes(Some(&parameterized), None));
        assert!(!wp_is_xml_request_bytes(None, Some(&parameterized)));
        assert!(!wp_is_xml_request_bytes(
            Some(&kind.to_ascii_uppercase()),
            None
        ));
    }
    assert!(!wp_is_xml_request_bytes(
        Some(b"application/xml"),
        Some(b"application/xml")
    ));
    assert!(wp_is_xml_request_bytes(Some(b"prefixtext/xmlsuffix"), None));
    assert!(!wp_is_xml_request_bytes(
        None,
        Some(b"prefixtext/xmlsuffix")
    ));
}

#[test]
fn mixed_hints_and_invalid_bytes_preserve_input_without_utf8_or_nul_rules() {
    let accept = b"\xff\0 text/xml;q=0, APPLICATION/JSON;q=0 \0".to_vec();
    let content_type = b"application/rss+xml\0".to_vec();
    let original_accept = accept.clone();
    let original_type = content_type.clone();
    assert!(wp_is_json_request_c_bytes(
        Some(&accept),
        Some(&content_type)
    ));
    assert!(wp_is_xml_request_bytes(Some(&accept), Some(&content_type)));
    assert_eq!(accept, original_accept);
    assert_eq!(content_type, original_type);
    assert!(!wp_is_xml_request_bytes(None, Some(b"text/xml\0")));
    assert!(wp_is_xml_request_bytes(Some(b"\0text/xml\xff"), None));
    assert!(!wp_is_json_request_c_bytes(
        Some(b"\0application/json"),
        None
    ));
    assert!(wp_is_json_request_c_bytes(
        Some(b"\xff,application/json;\0"),
        None
    ));
}

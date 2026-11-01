use recastpress_wp::{
    BadProtocolError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_kses_bad_protocol_once2_c_bytes as normalize,
};

fn protocols(values: &[&[u8]]) -> PhpArray {
    let mut protocols = PhpArray::new();
    for (index, value) in values.iter().enumerate() {
        protocols.insert(
            PhpKey::Integer(index as i64),
            PhpValue::Bytes(value.to_vec()),
        );
    }
    protocols
}

#[test]
fn normalization_order_decodes_then_removes_whitespace_before_slash_zero_runs() {
    let allowed = protocols(&[b"HTTP"]);
    for scheme in [
        b"HTTP".as_slice(),
        b"h t\nt\rp",
        b"h\0t\x1ftp",
        b"ht\\\t0tp",
        b"ht\\ 0tp",
        b"h&#116;tp",
        b"h&#38;#x74;tp",
    ] {
        let original = scheme.to_vec();
        assert_eq!(normalize(scheme, &allowed, 5), Ok(b"http:".to_vec()));
        assert_eq!(scheme, original);
    }
}

#[test]
fn allowed_protocols_are_only_ascii_case_compared_not_decoded_trimmed_or_filtered() {
    for (scheme, protocol) in [
        (b"ht tp".as_slice(), b"ht tp".as_slice()),
        (b"http", b" http"),
        (b"&#104;ttp", b"&#104;ttp"),
        (b"\x80 \0", b"\x80 \0"),
        ("Ä".as_bytes(), "ä".as_bytes()),
    ] {
        assert_eq!(
            normalize(scheme, &protocols(&[protocol]), 0),
            Ok(Vec::new())
        );
    }
    let mut allowed = protocols(&[b"not-http", b"HtTp"]);
    allowed.insert(
        PhpKey::Bytes(b"named-key".to_vec()),
        PhpValue::Bytes(b"HTTP".to_vec()),
    );
    let original = allowed.clone();
    assert_eq!(normalize(b"HTTP", &allowed, 5), Ok(b"http:".to_vec()));
    assert_eq!(allowed, original);
}

#[test]
fn empty_zero_colon_and_non_url_schemes_are_not_given_extra_grammar_rules() {
    for (scheme, protocol, expected) in [
        (b"".as_slice(), b"".as_slice(), b":".as_slice()),
        (b"0", b"0", b"0:"),
        (b"http:", b"HTTP:", b"http::"),
        (b"123_+/@", b"123_+/@", b"123_+/@:"),
        (b"\xff", b"\xff", b"\xff:"),
        (b"\0\\0", b"", b":"),
        (b"&#38;#256;", b"&#256;", b"&#256;:"),
        (b"&#x26;#x100;", b"&#x100;", b"&#x100;:"),
        (b"&#38;#\\000x100;", b"&#x100;", b"&#x100;:"),
    ] {
        assert_eq!(
            normalize(scheme, &protocols(&[protocol]), expected.len()),
            Ok(expected.to_vec())
        );
    }
    assert_eq!(normalize(b"", &PhpArray::new(), 0), Ok(Vec::new()));
}

#[test]
fn every_protocol_value_and_scheme_entity_domain_is_validated_even_for_nonmatches() {
    for value in [
        PhpValue::Null,
        PhpValue::Bool(false),
        PhpValue::Integer(0),
        PhpValue::Float(0.0),
        PhpValue::Array(PhpArray::new()),
        PhpValue::Unsupported,
    ] {
        let mut allowed = protocols(&[b"http"]);
        allowed.insert(PhpKey::Integer(1), value);
        assert_eq!(
            normalize(b"http", &allowed, 5),
            Err(BadProtocolError::UnsupportedProtocolValue)
        );
    }
    for scheme in [b"&#256;".as_slice(), b"&#38;#x100;"] {
        assert_eq!(
            normalize(scheme, &PhpArray::new(), 0),
            Err(BadProtocolError::UnsupportedEntityValue)
        );
    }
    assert_eq!(
        normalize(b"&#256x;", &protocols(&[b"&#256x;"]), 8),
        Ok(b"&#256x;:".to_vec())
    );
}

#[test]
fn final_cap_includes_colon_only_after_match_and_never_limits_shrinking_work_buffers() {
    let mut scheme = vec![b' '; 4096];
    scheme.extend_from_slice(b"h&#116;tp");
    assert_eq!(
        normalize(&scheme, &protocols(&[b"http"]), 5),
        Ok(b"http:".to_vec())
    );
    assert_eq!(
        normalize(&scheme, &protocols(&[b"http"]), 4),
        Err(BadProtocolError::OutputLimit)
    );
    assert_eq!(normalize(&scheme, &PhpArray::new(), 0), Ok(Vec::new()));
    assert_eq!(
        normalize(b"", &protocols(&[b""]), 0),
        Err(BadProtocolError::OutputLimit)
    );
    assert_eq!(
        normalize(&vec![b'x'; 1048577], &PhpArray::new(), 0),
        Err(BadProtocolError::InputLimit)
    );
}

#[test]
fn all_bytes_preserve_c_whitespace_and_ascii_case_behavior_without_utf8_sanitizing() {
    for byte in 0..=255u8 {
        let allowed = protocols(&[&[byte]]);
        let expected = if byte <= 32 {
            Vec::new()
        } else {
            vec![byte.to_ascii_lowercase(), b':']
        };
        assert_eq!(normalize(&[byte], &allowed, expected.len()), Ok(expected));
    }
}

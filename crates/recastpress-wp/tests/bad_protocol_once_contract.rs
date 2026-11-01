use recastpress_wp::{
    BadProtocolOnceError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_kses_bad_protocol_once_c_bytes as once,
};

fn protocols(values: &[&[u8]]) -> PhpArray {
    let mut result = PhpArray::new();
    for (index, value) in values.iter().enumerate() {
        result.insert(
            PhpKey::Integer(index as i64),
            PhpValue::Bytes(value.to_vec()),
        );
    }
    result
}

#[test]
fn delimiter_repair_preserves_case_and_lookahead_boundaries_then_splits_only_once() {
    for (input, expected) in [
        (b"HTTP: x".as_slice(), b"http:x".as_slice()),
        (b"http:a:b", b"http:a:b"),
        (b"http&#058x", b"http:x"),
        (b"http&#0589x", b"http&#0589x"),
        (b"http&#58", b"http:"),
        (b"http&#X003AZ", b"http:Z"),
        (b"http&#x3af", b"http&#x3af"),
        (b"http&#x3ag", b"http:g"),
        (b"http&COLON;x", b"http:x"),
        (b"http&colonx", b"http&colonx"),
        (b"http:a&#58b", b"http:a&#58;b"),
        (b"bad:http:x", b"http:x"),
    ] {
        assert_eq!(
            once(input, &protocols(&[b"http"]), expected.len()),
            Ok(expected.to_vec())
        );
    }
}

#[test]
fn raw_question_guard_and_noncalled_body_preserve_out_of_domain_entity_text() {
    for (input, expected) in [
        (b"plain&#256;".as_slice(), b"plain&#256;".as_slice()),
        (b"http:&#256;", b"http:&#256;"),
        (b"raw/?&#256;:rest", b"raw/?&#256;:rest"),
        (b"raw/?path&#58x", b"raw/?path&#58;x"),
        (b"&#47;?x:http:a", b"http:a"),
        (b"/\0?x:http:a", b"http:a"),
        (b"bad:&#256;:rest", b"&#256;:rest"),
    ] {
        assert_eq!(
            once(input, &protocols(&[b"http"]), expected.len()),
            Ok(expected.to_vec())
        );
    }
}

#[test]
fn suffix_trim_has_php_six_byte_semantics_and_does_not_change_unsplit_input() {
    let allowed = protocols(&[b"http"]);
    assert_eq!(
        once(b"http:\0 \t\n\x0b\rX\0 \t\n\x0b\r", &allowed, 6),
        Ok(b"http:X".to_vec())
    );
    assert_eq!(
        once(b"http:\x0cX\x0c", &allowed, 8),
        Ok(b"http:\x0cX\x0c".to_vec())
    );
    assert_eq!(
        once(b" \0plain\x0c ", &allowed, 10),
        Ok(b" \0plain\x0c ".to_vec())
    );
    assert_eq!(
        once(b"\xff:http:\0x", &allowed, 7),
        Ok(b"http:\0x".to_vec())
    );
}

#[test]
fn default_feed_depth_and_php_empty_zero_preserve_source_prefix_rules() {
    let allowed = protocols(&[b"feed", b"http"]);
    for (input, expected) in [
        (b"feed:http:x".as_slice(), b"feed:http:x".as_slice()),
        (b"feed:feed:http:x", b"feed:feed:http:x"),
        (b"feed:feed:feed:http:x", b""),
        (b"feed:feed:feed:&#256;:x", b""),
        (b"feed:", b""),
        (b"feed:0", b"0"),
        (b"feed:00", b"feed:00"),
        (b"feed:feed:0", b"0"),
        (b"feed:bad:javascript:x", b"feed:javascript:x"),
    ] {
        assert_eq!(once(input, &allowed, expected.len()), Ok(expected.to_vec()));
    }
}

#[test]
fn only_called_prefixes_validate_byte_entity_domain_even_in_the_third_frame() {
    let allowed = protocols(&[b"feed", b"http"]);
    for input in [
        b"&#256;:x".as_slice(),
        b"feed:&#256;:x",
        b"feed:feed:&#256;:x",
        b"&#38;#x100;:x",
    ] {
        assert_eq!(
            once(input, &allowed, 0),
            Err(BadProtocolOnceError::UnsupportedEntityValue)
        );
    }
    assert_eq!(
        once(b"feed:&#256;", &allowed, 12),
        Ok(b"feed:&#256;".to_vec())
    );
    let mut invalid = protocols(&[b"http"]);
    invalid.insert(PhpKey::Integer(1), PhpValue::Null);
    assert_eq!(
        once(b"no delimiter", &invalid, 100),
        Err(BadProtocolOnceError::UnsupportedProtocolValue)
    );
}

#[test]
fn top_input_bound_is_not_reapplied_to_expanded_repair_or_recursive_tails() {
    let count = (1048576 - 8) / 4;
    let mut input = b"feed:".to_vec();
    input.extend_from_slice(&b"&#58".repeat(count));
    input.extend_from_slice(b"xxx");
    assert_eq!(input.len(), 1048576);
    let mut expected = b"feed:".to_vec();
    expected.extend_from_slice(&b"&#58;".repeat(count - 1));
    expected.extend_from_slice(b"xxx");
    assert!(expected.len() > 1048576);
    assert_eq!(
        once(&input, &protocols(&[b"feed"]), expected.len()),
        Ok(expected)
    );
    input.push(b'x');
    assert_eq!(
        once(&input, &protocols(&[b"feed"]), usize::MAX),
        Err(BadProtocolOnceError::InputLimit)
    );
}

#[test]
fn top_final_cap_is_checked_after_feed_collapse_and_prefix_working_results() {
    let allowed = protocols(&[b"feed", b"http"]);
    assert_eq!(once(b"feed:", &allowed, 0), Ok(Vec::new()));
    assert_eq!(once(b"feed:feed:feed:http:x", &allowed, 0), Ok(Vec::new()));
    assert_eq!(once(b"feed:0", &allowed, 1), Ok(b"0".to_vec()));
    assert_eq!(
        once(b"feed:0", &allowed, 0),
        Err(BadProtocolOnceError::OutputLimit)
    );
    let input = b"HTTP: x".to_vec();
    let original = input.clone();
    assert_eq!(once(&input, &allowed, 6), Ok(b"http:x".to_vec()));
    assert_eq!(
        once(&input, &allowed, 5),
        Err(BadProtocolOnceError::OutputLimit)
    );
    assert_eq!(input, original);
}

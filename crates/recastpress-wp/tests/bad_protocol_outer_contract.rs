use recastpress_wp::{
    BadProtocolOuterError,
    value::{PhpArray, PhpKey, PhpValue},
    wp_kses_bad_protocol_c_bytes as outer,
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
fn global_no_null_precedes_fastpath_and_filters_the_whole_body_once() {
    let input = b"h\0ttp://x\\0y \xff&#256;".to_vec();
    let original = input.clone();
    let allowed = protocols(&[b"http"]);
    let original_allowed = allowed.clone();
    let expected = b"http://xy \xff&#256;";
    assert_eq!(
        outer(&input, &allowed, expected.len()),
        Ok(expected.to_vec())
    );
    assert_eq!(input, original);
    assert_eq!(allowed, original_allowed);
    assert_eq!(
        outer(b"/\0?x:http:x", &allowed, 11),
        Ok(b"/?x:http:x".to_vec())
    );
}

#[test]
fn fastpath_is_literal_lowercase_prefix_and_strict_lowercase_protocol_value() {
    let raw = b"http://x?a&#58b ";
    assert_eq!(
        outer(raw, &protocols(&[b"http"]), raw.len()),
        Ok(raw.to_vec())
    );
    assert_eq!(
        outer(raw, &protocols(&[b"HTTP"]), 17),
        Ok(b"http://x?a&#58;b".to_vec())
    );
    assert_eq!(
        outer(b"HTTP://x?a&#58b ", &protocols(&[b"http"]), 17),
        Ok(b"http://x?a&#58;b".to_vec())
    );
    assert_eq!(
        outer(b"https://x\t", &protocols(&[b"https"]), 10),
        Ok(b"https://x\t".to_vec())
    );
    assert_eq!(
        outer(b"httpx://x", &protocols(&[b"http"]), 3),
        Ok(b"//x".to_vec())
    );
    assert_eq!(
        outer(b"http://x", &protocols(&[b"http:"]), 3),
        Ok(b"//x".to_vec())
    );
}

#[test]
fn six_pass_cutoff_compares_strict_bytes_and_requires_stability_on_the_last_pass() {
    let allowed = protocols(&[b"http"]);
    for (count, tail, expected) in [
        (5, b"body".as_slice(), b"body".as_slice()),
        (6, b"body", b""),
        (5, b"http:x", b"http:x"),
        (5, b"HTTP:x", b""),
        (4, b"HTTP:x", b"http:x"),
    ] {
        let mut input = b"bad:".repeat(count);
        input.extend_from_slice(tail);
        assert_eq!(
            outer(&input, &allowed, expected.len()),
            Ok(expected.to_vec())
        );
    }
}

#[test]
fn entity_domain_is_conditional_on_the_exact_visited_prefixes_and_loop_cutoff() {
    let allowed = protocols(&[b"http"]);
    for input in [b"plain&#256;".as_slice(), b"http:&#256;", b"raw/?&#256;:x"] {
        assert_eq!(outer(input, &allowed, input.len()), Ok(input.to_vec()));
    }
    assert_eq!(
        outer(b"&#2\0\x35\x36;:x", &allowed, 0),
        Err(BadProtocolOuterError::UnsupportedEntityValue)
    );
    let mut called = b"bad:".repeat(5);
    called.extend_from_slice(b"&#256;:x");
    assert_eq!(
        outer(&called, &allowed, 0),
        Err(BadProtocolOuterError::UnsupportedEntityValue)
    );
    let mut skipped = b"bad:".repeat(6);
    skipped.extend_from_slice(b"&#256;:x");
    assert_eq!(outer(&skipped, &allowed, 0), Ok(Vec::new()));
}

#[test]
fn each_once_resets_feed_count_and_preserves_php_empty_zero_results() {
    let allowed = protocols(&[b"http", b"feed"]);
    assert_eq!(outer(b"feed:0", &allowed, 1), Ok(b"0".to_vec()));
    assert_eq!(outer(b"feed:00", &allowed, 7), Ok(b"feed:00".to_vec()));
    assert_eq!(
        outer(b"bad:feed:feed:http:x", &allowed, 16),
        Ok(b"feed:feed:http:x".to_vec())
    );
    assert_eq!(outer(b"feed:feed:feed:http:x", &allowed, 0), Ok(Vec::new()));
}

#[test]
fn expanded_working_inputs_bypass_public_once_top_guard_during_internal_iteration() {
    let allowed = protocols(&[b"http"]);
    let mut input = b"bad:".to_vec();
    input.extend_from_slice(&b"&#58".repeat((1048576 - 4) / 4));
    assert_eq!(input.len(), 1048576);
    assert_eq!(outer(&input, &allowed, 0), Ok(Vec::new()));
    let count = (1048576 - 8) / 4;
    let mut guarded = b"raw/?".to_vec();
    guarded.extend_from_slice(&b"&#58".repeat(count));
    guarded.extend_from_slice(b"xxx");
    let mut expected = b"raw/?".to_vec();
    expected.extend_from_slice(&b"&#58;".repeat(count));
    expected.extend_from_slice(b"xxx");
    assert!(expected.len() > 1048576);
    assert_eq!(outer(&guarded, &allowed, expected.len()), Ok(expected));
}

#[test]
fn top_input_and_final_caps_are_separate_from_all_working_allocations() {
    let allowed = protocols(&[b"http"]);
    assert_eq!(
        outer(&vec![0; 1048577], &allowed, 0),
        Err(BadProtocolOuterError::InputLimit)
    );
    let input = b"http://x";
    assert_eq!(outer(input, &allowed, input.len()), Ok(input.to_vec()));
    assert_eq!(
        outer(input, &allowed, input.len() - 1),
        Err(BadProtocolOuterError::OutputLimit)
    );
    assert_eq!(
        outer(b"feed:0", &protocols(&[b"feed"]), 0),
        Err(BadProtocolOuterError::OutputLimit)
    );
    let mut invalid = protocols(&[b"http"]);
    invalid.insert(PhpKey::Integer(1), PhpValue::Null);
    assert_eq!(
        outer(input, &invalid, 100),
        Err(BadProtocolOuterError::UnsupportedProtocolValue)
    );
}

#[test]
fn global_control_filter_does_not_remove_whitespace_or_sanitize_high_bytes() {
    for byte in 0..=255u8 {
        let expected = if matches!(byte, 0..=8 | 11 | 12 | 14..=31 | b':') {
            Vec::new()
        } else {
            vec![byte]
        };
        assert_eq!(
            outer(&[byte], &PhpArray::new(), expected.len()),
            Ok(expected)
        );
    }
}

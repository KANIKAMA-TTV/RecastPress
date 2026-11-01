use recastpress_wp::{
    TruncatePostSlugError, truncate_post_slug_conditional_decoded_utf8_bytes as truncate,
};
use std::error::Error;

#[test]
fn three_branches_and_zero_length_keep_their_distinct_source_behavior() {
    assert_eq!(truncate(b"abc-", 4, 3).unwrap(), b"abc");
    assert_eq!(truncate(b"abcdef", 3, 3).unwrap(), b"abc");
    assert_eq!(truncate(b"%41BC", 3, 3).unwrap(), b"ABC");
    assert_eq!(truncate(b"abc", 0, 0).unwrap(), b"");
    assert_eq!(truncate(b"%41", 0, 1).unwrap(), b"A");
    assert_eq!(truncate(b"", 0, 0).unwrap(), b"");
}

#[test]
fn unguarded_raw_branches_preserve_invalid_bytes_and_byte_cut_utf8() {
    assert_eq!(truncate(b"\xff-", 2, 1).unwrap(), b"\xff");
    assert_eq!(truncate(b"\xffabc", 1, 1).unwrap(), b"\xff");
    assert_eq!(truncate("éX".as_bytes(), 1, 1).unwrap(), b"\xc3");
    assert_eq!(truncate(b"%ff", 3, 3).unwrap(), b"%ff");
    assert_eq!(truncate(b"%zXY", 3, 3).unwrap(), b"%zX");
}

#[test]
fn url_decode_is_nonrecursive_and_reencoding_preserves_source_hex_case_rules() {
    assert_eq!(truncate(b"%2541", 0, 5).unwrap(), b"%2541");
    assert_eq!(truncate(b"%2b+", 0, 6).unwrap(), b"%2B%20");
    assert_eq!(truncate(b"%c3%A9%2f", 0, 9).unwrap(), b"%c3%a9%2F");
    assert_eq!(truncate(b"%00", 0, 3).unwrap(), b"%00");
    assert_eq!(truncate(b"%E2%82%ACX", 8, 0).unwrap(), b"");
    assert_eq!(truncate(b"%E2%82%ACX", 9, 9).unwrap(), b"%e2%82%ac");
}

#[test]
fn final_hyphen_trim_is_after_branch_processing_and_before_host_cap() {
    assert_eq!(truncate(b"a---", 3, 1).unwrap(), b"a");
    assert_eq!(truncate(b"--a--", usize::MAX, 3).unwrap(), b"--a");
    assert_eq!(truncate(b"- \t", 3, 3).unwrap(), b"- \t");
    assert_eq!(truncate(&b"%2d".repeat(1024), 0, 0).unwrap(), b"");
    assert_eq!(truncate(b"%41%2d", 0, 1).unwrap(), b"A");
    assert_eq!(
        truncate(b"%41%2d", 0, 0),
        Err(TruncatePostSlugError::OutputLimit)
    );
}

#[test]
fn changed_decoding_requires_whole_utf8_even_beyond_the_source_budget() {
    for raw in [b"%ff".as_slice(), b"A%ff", b"%c3", b"%ed%a0%80", b"+\xff"] {
        assert_eq!(
            truncate(raw, 1, 0),
            Err(TruncatePostSlugError::UnsupportedValue)
        );
    }
    assert_eq!(truncate(b"A%ff", 4, 4).unwrap(), b"A%ff");
}

#[test]
fn input_is_immutable_and_errors_never_capture_private_payloads() {
    let raw = b"PRIVATE%2B+%c3%a9---".to_vec();
    let saved = raw.clone();
    let output = truncate(&raw, 0, usize::MAX).unwrap();
    assert_eq!(raw, saved);
    assert_eq!(truncate(&raw, 0, output.len()).unwrap(), output);
    assert_eq!(
        truncate(&raw, 0, output.len() - 1),
        Err(TruncatePostSlugError::OutputLimit)
    );
    for error in [
        TruncatePostSlugError::UnsupportedValue,
        TruncatePostSlugError::OutputLimit,
        TruncatePostSlugError::AllocationFailed,
    ] {
        assert!(error.source().is_none());
        assert!(!format!("{error:?}: {error}").contains("PRIVATE"));
    }
}

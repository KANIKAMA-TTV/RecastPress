use recastpress_wp::{Utf8UriEncodeError, utf8_uri_encode_utf8_bytes as encode};
use std::error::Error;

#[test]
fn all_ascii_is_raw_or_rawurlencode_uppercase_with_the_exact_unreserved_set() {
    let input = (0u8..128).collect::<Vec<_>>();
    let saved = input.clone();
    assert_eq!(encode(&input, 0, false, usize::MAX), Ok(input.clone()));
    let expected=b"%00%01%02%03%04%05%06%07%08%09%0A%0B%0C%0D%0E%0F%10%11%12%13%14%15%16%17%18%19%1A%1B%1C%1D%1E%1F%20%21%22%23%24%25%26%27%28%29%2A%2B%2C-.%2F0123456789%3A%3B%3C%3D%3E%3F%40ABCDEFGHIJKLMNOPQRSTUVWXYZ%5B%5C%5D%5E_%60abcdefghijklmnopqrstuvwxyz%7B%7C%7D~%7F";
    assert_eq!(encode(&input, 0, true, usize::MAX).unwrap(), expected);
    assert_eq!(input, saved);
}

#[test]
fn unicode_scalars_encode_each_utf8_byte_in_lowercase_independently_of_ascii_mode() {
    let input = "é €🦀 %";
    assert_eq!(
        encode(input.as_bytes(), 0, false, usize::MAX).unwrap(),
        b"%c3%a9 %e2%82%ac%f0%9f%a6%80 %"
    );
    assert_eq!(
        encode(input.as_bytes(), 0, true, usize::MAX).unwrap(),
        b"%c3%a9%20%e2%82%ac%f0%9f%a6%80%20%25"
    );
    for (input, expected) in [
        ("\u{80}", b"%c2%80".as_slice()),
        ("\u{7ff}", b"%df%bf"),
        ("\u{800}", b"%e0%a0%80"),
        ("\u{ffff}", b"%ef%bf%bf"),
        ("\u{10000}", b"%f0%90%80%80"),
        ("\u{10ffff}", b"%f4%8f%bf%bf"),
    ] {
        assert_eq!(
            encode(input.as_bytes(), 0, false, usize::MAX).unwrap(),
            expected
        );
    }
}

#[test]
fn source_length_uses_whole_tokens_and_stops_instead_of_filling_with_later_characters() {
    for (input, size) in [("é", 6), ("€", 9), ("🦀", 12)] {
        let raw = format!("{input}A");
        assert_eq!(encode(raw.as_bytes(), size - 1, false, 0), Ok(Vec::new()));
        assert_eq!(
            encode(raw.as_bytes(), size, false, usize::MAX).unwrap(),
            encode(input.as_bytes(), 0, false, usize::MAX).unwrap()
        );
    }
    assert_eq!(encode(b"!A", 2, true, 0), Ok(Vec::new()));
    assert_eq!(encode(b"!A", 3, true, 3), Ok(b"%21".to_vec()));
    assert_eq!(encode(b"!A", 1, false, 1), Ok(b"!".to_vec()));
    assert_eq!(encode("aéZ".as_bytes(), 6, false, 1), Ok(b"a".to_vec()));
    assert_eq!(
        encode("aéZ".as_bytes(), 7, false, 7),
        Ok(b"a%c3%a9".to_vec())
    );
}

#[test]
fn source_budget_and_host_final_cap_are_distinct_with_empty_and_nul_boundaries() {
    assert_eq!(encode(b"", 0, true, 0), Ok(Vec::new()));
    assert_eq!(encode(b"\0", 0, false, 1), Ok(vec![0]));
    assert_eq!(encode(b"\0", 2, true, 0), Ok(Vec::new()));
    assert_eq!(encode(b"\0", 3, true, 3), Ok(b"%00".to_vec()));
    assert_eq!(
        encode(b"\0", 3, true, 2),
        Err(Utf8UriEncodeError::OutputLimit)
    );
    assert_eq!(encode("éXYZ".as_bytes(), 5, false, 0), Ok(Vec::new()));
    assert_eq!(
        encode("éXYZ".as_bytes(), 7, false, 7),
        Ok(b"%c3%a9X".to_vec())
    );
    assert_eq!(
        encode("éXYZ".as_bytes(), 7, false, 6),
        Err(Utf8UriEncodeError::OutputLimit)
    );
    assert_eq!(
        encode(b"A", 0, false, 0),
        Err(Utf8UriEncodeError::OutputLimit)
    );
}

#[test]
fn whole_utf8_validation_rejects_even_invalid_trailing_bytes_beyond_the_source_budget() {
    for raw in [
        b"A\xff".as_slice(),
        b"A\xc3",
        b"A\xc0\xaf",
        b"A\xed\xa0\x80",
        b"A\xf4\x90\x80\x80",
    ] {
        for encode_ascii in [false, true] {
            assert_eq!(
                encode(raw, 1, encode_ascii, 0),
                Err(Utf8UriEncodeError::UnsupportedValue)
            );
        }
    }
    let input = b" \0\xc3\xa9/";
    let saved = input.to_vec();
    assert_eq!(encode(input, 0, false, usize::MAX).unwrap(), b" \0%c3%a9/");
    assert_eq!(input.as_slice(), saved);
}

#[test]
fn long_prefix_and_usize_max_limits_preserve_source_break_and_fixed_private_diagnostics() {
    let raw = format!("{}AéZ", "é".repeat(33));
    let expected = format!("{}A", "%c3%a9".repeat(33)).into_bytes();
    assert_eq!(encode(raw.as_bytes(), 200, false, 199), Ok(expected));
    assert_eq!(
        encode("é".as_bytes(), usize::MAX, false, usize::MAX),
        encode("é".as_bytes(), 0, false, usize::MAX)
    );
    for error in [
        Utf8UriEncodeError::UnsupportedValue,
        Utf8UriEncodeError::OutputLimit,
        Utf8UriEncodeError::AllocationFailed,
    ] {
        assert!(error.source().is_none());
        assert!(!format!("{error:?}: {error}").contains("PRIVATE_INPUT"));
    }
}

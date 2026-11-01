use recastpress_wp::{StripAllTagsError, wp_strip_all_tags_no_open_angle_bytes as strip};

#[test]
fn no_open_angle_preserves_greater_entities_invalid_utf8_and_deletes_every_nul() {
    let raw = b" \0> &lt;script&gt;\xff\xc0\xaf\0X\0 ";
    let saved = raw.to_vec();
    assert_eq!(
        strip(raw, false, usize::MAX).unwrap(),
        b"> &lt;script&gt;\xff\xc0\xafX"
    );
    assert_eq!(raw.as_slice(), saved);
    assert_eq!(strip(b"0", false, 1).unwrap(), b"0");
    assert_eq!(
        strip(b"<", false, 0),
        Err(StripAllTagsError::UnsupportedValue)
    );
}

#[test]
fn optional_breaks_only_collapse_four_bytes_before_exact_php_trim_six() {
    let raw = b"\x0b \tA \0\t\r\nB\x0bC\x0cD \x0b";
    assert_eq!(
        strip(raw, false, usize::MAX).unwrap(),
        b"A \t\r\nB\x0bC\x0cD"
    );
    assert_eq!(strip(raw, true, usize::MAX).unwrap(), b"A B\x0bC\x0cD");
    assert_eq!(
        strip(b"\x0c\t A\r\n\x0c", true, usize::MAX).unwrap(),
        b"\x0c A \x0c"
    );
    assert_eq!(strip(b"\0 \t\r\n\x0b", true, 0), Ok(Vec::new()));
}

#[test]
fn final_cap_allows_empty_from_arbitrarily_larger_input_and_rejects_no_partial_output() {
    assert_eq!(strip(&[0; 4097], false, 0), Ok(Vec::new()));
    assert_eq!(strip(b"", false, 0), Ok(Vec::new()));
    assert_eq!(
        strip(b"\x0c", false, 0),
        Err(StripAllTagsError::OutputLimit)
    );
    assert_eq!(strip(b" A \0\t B ", true, 3), Ok(b"A B".to_vec()));
    assert_eq!(
        strip(b" A \0\t B ", true, 2),
        Err(StripAllTagsError::OutputLimit)
    );
}

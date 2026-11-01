use recastpress_wp::normalize_whitespace_bytes;

#[test]
fn trims_exactly_php_default_bytes_at_both_edges() {
    assert_eq!(normalize_whitespace_bytes(b""), b"");
    assert_eq!(normalize_whitespace_bytes(b" \t\n\r\0\x0b"), b"");
    for byte in [b' ', b'\t', b'\n', b'\r', 0, 0x0b] {
        assert_eq!(normalize_whitespace_bytes(&[byte, b'x', byte]), b"x");
    }
    // Form feed stops trimming, so whitespace behind it remains interior.
    assert_eq!(
        normalize_whitespace_bytes(b" \x0c \tx \t\x0c "),
        b"\x0c x \x0c"
    );
    assert_eq!(
        normalize_whitespace_bytes("\u{a0} \tx \t\u{2003}".as_bytes()),
        "\u{a0} x \u{2003}".as_bytes()
    );
}

#[test]
fn replaces_cr_before_collapsing_newlines_and_retains_line_spaces() {
    let input = b" \r\n\talpha\r\n\r\r\nbeta \t \r\n \t gamma\n\r delta\r\n ";
    let original = input.to_vec();
    assert_eq!(
        normalize_whitespace_bytes(input),
        b"alpha\nbeta \n gamma\n delta"
    );
    assert_eq!(input.as_slice(), original);
    // Space separates newline runs and is not stripped independently per line.
    assert_eq!(normalize_whitespace_bytes(b"a\n \t\n\r\nb"), b"a\n \nb");
}

#[test]
fn preserves_interior_controls_non_utf8_and_literal_output_bytes() {
    let input = b"\0 \x0ba\0\x0bb\x0cc\xff\xc3\xa9\t\t<&>\0z\x0b\0 ".to_vec();
    let original = input.clone();
    let normalized = normalize_whitespace_bytes(&input);
    assert_eq!(normalized, b"a\0\x0bb\x0cc\xff\xc3\xa9 <&>\0z");
    assert_eq!(input, original);
    assert_eq!(normalize_whitespace_bytes(&normalized), normalized);
    assert_eq!(
        normalize_whitespace_bytes(b"\xff\0\x0b\x0c\xff"),
        b"\xff\0\x0b\x0c\xff"
    );
}

use recastpress_wp::wp_make_link_relative_bytes;

#[test]
fn link_relative_strips_only_optional_http_schemes_and_nonempty_authorities() {
    for input in [
        b"http://host/path".as_slice(),
        b"HtTp://host/path",
        b"HTTPS://host/path",
        b"//host/path",
    ] {
        assert_eq!(wp_make_link_relative_bytes(input), b"/path");
    }
    for input in [
        b"".as_slice(),
        b"/path",
        b"relative/path",
        b"ftp://host/path",
        b"https:host/path",
        b" //host/path",
        b"//",
        b"///path",
        b"http://",
        b"https:///path",
    ] {
        assert_eq!(wp_make_link_relative_bytes(input), input);
    }
}

#[test]
fn link_relative_keeps_the_regex_authority_rule_instead_of_url_parsing() {
    for input in [
        b"https://host".as_slice(),
        b"//host?query#fragment",
        b"http://user:password@host:8080",
    ] {
        assert_eq!(wp_make_link_relative_bytes(input), b"");
    }
    assert_eq!(
        wp_make_link_relative_bytes(b"https://host?redirect=/path#fragment"),
        b"/path#fragment"
    );
    assert_eq!(wp_make_link_relative_bytes(b"//host//path"), b"//path");
    assert_eq!(
        wp_make_link_relative_bytes(b"https://host/a?b#c"),
        b"/a?b#c"
    );
}

#[test]
fn link_relative_preserves_newline_suffixes_and_does_not_reapply_the_pattern() {
    for input in [
        b"https://host/path\nrest".as_slice(),
        b"//host\n/path\nrest",
    ] {
        assert_eq!(wp_make_link_relative_bytes(input), b"/path\nrest");
    }
    assert_eq!(wp_make_link_relative_bytes(b"//host\nrest"), b"");
    assert_eq!(wp_make_link_relative_bytes(b"//host\nrest/path"), b"/path");
    assert_eq!(
        wp_make_link_relative_bytes(b"http://host/a\nhttp://other/b"),
        b"/a\nhttp://other/b"
    );
    assert_eq!(
        wp_make_link_relative_bytes(b"https://host/path\r\nrest"),
        b"/path\r\nrest"
    );
}

#[test]
fn link_relative_keeps_arbitrary_tail_bytes_and_does_not_mutate_input() {
    for byte in 0..=255u8 {
        let input = [b"http://host/".as_slice(), &[byte], b"\0\xff"].concat();
        let before = input.clone();
        assert_eq!(
            wp_make_link_relative_bytes(&input),
            [b"/".as_slice(), &[byte], b"\0\xff"].concat()
        );
        assert_eq!(input, before);
    }
    assert_eq!(wp_make_link_relative_bytes(b"//\0\xff/path"), b"/path");
    assert_eq!(
        wp_make_link_relative_bytes(b"//host/\xe6\x97\xa5\xff"),
        b"/\xe6\x97\xa5\xff"
    );
}

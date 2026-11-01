use recastpress_wp::wp_html_split_bytes;

fn check(input: &[u8], expected: &[&[u8]]) {
    assert_eq!(wp_html_split_bytes(input), expected);
}

#[test]
fn html_split_retains_captured_delimiters_and_empty_text_entries() {
    check(b"", &[b""]);
    check(b"plain text", &[b"plain text"]);
    check(b"<a>", &[b"", b"<a>", b""]);
    check(b"<a></a>", &[b"", b"<a>", b"", b"</a>", b""]);
    check(b"x<a>y</a>z", &[b"x", b"<a>", b"y", b"</a>", b"z"]);
}

#[test]
fn html_split_uses_exact_comment_and_cdata_prefixes_and_first_terminators() {
    check(b"a<!-- x > <b> -->z", &[b"a", b"<!-- x > <b> -->", b"z"]);
    check(
        b"<![CDATA[x > <b> ]]>z",
        &[b"", b"<![CDATA[x > <b> ]]>", b"z"],
    );
    check(b"<!-->after", &[b"", b"<!-->", b"after"]);
    check(b"<!--x--->after", &[b"", b"<!--x--->", b"after"]);
    check(b"<![CDATA[]]]>after", &[b"", b"<![CDATA[]]]>", b"after"]);
    check(b"<!--a-->b-->c", &[b"", b"<!--a-->", b"b-->c"]);
    check(b"<![cdata[x>y", &[b"", b"<![cdata[x>", b"y"]);
    check(b"<!-x>y", &[b"", b"<!-x>", b"y"]);
}

#[test]
fn html_split_consumes_unclosed_delimiters_through_end_of_input() {
    for input in [
        b"<".as_slice(),
        b"<a",
        b"<!--",
        b"<!--x><a>",
        b"<![CDATA[",
        b"<![CDATA[x><a>",
    ] {
        check(input, &[b"", input, b""]);
    }
    check(b"text<a", &[b"text", b"<a", b""]);
}

#[test]
fn html_split_is_a_byte_regex_split_rather_than_an_html_parser() {
    check(b"<a title='>'>x", &[b"", b"<a title='>", b"'>x"]);
    check(b"<<a>tail", &[b"", b"<<a>", b"tail"]);
    check(b"x < y > z", &[b"x ", b"< y >", b" z"]);
    check(b"a\0<\xff>\xfe", &[b"a\0", b"<\xff>", b"\xfe"]);
    check(b"<!--\0\xff>-->\xfe", &[b"", b"<!--\0\xff>-->", b"\xfe"]);
}

#[test]
fn html_split_preserves_every_byte_and_alternation_across_all_byte_values() {
    for byte in 0..=255u8 {
        for input in [
            vec![byte],
            vec![byte, b'<', byte, b'>', byte],
            vec![b'<', b'!', b'-', b'-', byte, b'-', b'-', b'>', byte],
            vec![b'<', b'!', b'[', b'C', b'D', b'A', b'T', b'A', b'[', byte],
        ] {
            let parts = wp_html_split_bytes(&input);
            assert_eq!(parts.concat(), input);
            assert_eq!(parts.len() % 2, 1);
            for delimiter in parts.iter().skip(1).step_by(2) {
                assert_eq!(delimiter.first(), Some(&b'<'));
            }
        }
    }
}

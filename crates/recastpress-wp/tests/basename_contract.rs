use recastpress_wp::wp_basename_bytes;

#[test]
fn basename_keeps_bytes_and_both_separator_spellings() {
    for (path, expected) in [
        (
            b"C:\\folder\\name.txt\\/".as_slice(),
            b"name.txt".as_slice(),
        ),
        (b"/dir/\0\xff +%2F", b"\0\xff +%2F"),
        (b"////\\", b""),
        (b"", b""),
        (b"../..", b".."),
        ("/日本語/🦀.txt".as_bytes(), "🦀.txt".as_bytes()),
    ] {
        assert_eq!(wp_basename_bytes(path, b""), expected);
    }
}

#[test]
fn suffix_matches_encoded_spelling_without_removing_the_entire_basename() {
    for (path, suffix, expected) in [
        (
            b"/file.txt".as_slice(),
            b".txt".as_slice(),
            b"file".as_slice(),
        ),
        (b"/file.txt", b"file.txt", b"file.txt"),
        (b"/x ", b" ", b"x "),
        (b"/x ", b"+", b"x"),
        (b"/x\xff", b"%FF", b"x"),
        (b"/x\xff", b"F", b"x%F"),
        (b"/x\xff", b"FF", b"x%"),
        (b"/x\xff", b"\xff", b"x\xff"),
    ] {
        assert_eq!(wp_basename_bytes(path, suffix), expected);
    }
}

use recastpress_wp::{
    CompatibilityError, trailingslashit_bytes, untrailingslashit_bytes, wp_normalize_path_bytes,
};

#[test]
fn trailing_slash_transforms_keep_nul_whitespace_and_byte_values() {
    assert_eq!(untrailingslashit_bytes(b"\\//"), b"");
    assert_eq!(trailingslashit_bytes(b"\\//"), b"/");
    assert_eq!(untrailingslashit_bytes(b"a\xff\0/\\"), b"a\xff\0");
    assert_eq!(trailingslashit_bytes(b"a\xff\0/\\"), b"a\xff\0/");
    assert_eq!(trailingslashit_bytes(b"a/ "), b"a/ /");
}

#[test]
fn normalize_path_retains_network_prefix_lf_and_unresolved_components() {
    for (input, expected) in [
        (
            b"c:\\path\\\\to//file".as_slice(),
            b"C:/path/to/file".as_slice(),
        ),
        (b"\\\\\\server\\share\\file", b"//server/share/file"),
        (b"a/./b/../c", b"a/./b/../c"),
        (b"a\n///b\r///c", b"a\n//b\r/c"),
        (b"\xff:\\\\a\0//b", b"\xff:/a\0/b"),
    ] {
        assert_eq!(wp_normalize_path_bytes(input).unwrap(), expected);
    }
}

#[test]
fn normalize_path_requires_explicit_future_stream_registry_support() {
    for value in [
        b"file:///tmp".as_slice(),
        b"unknown://x",
        b"://",
        b"x\0://y",
    ] {
        assert_eq!(
            wp_normalize_path_bytes(value),
            Err(CompatibilityError::UnsupportedValue)
        );
    }
    // Source tests for :// before changing backslashes, so this is not a stream.
    assert_eq!(
        wp_normalize_path_bytes(b"http:\\\\host").unwrap(),
        b"http:/host"
    );
}

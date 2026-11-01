use recastpress_wp::{wp_has_noncharacters_bytes, wp_is_valid_utf8_bytes, wp_scrub_utf8_bytes};

#[test]
fn well_formed_boundaries_bom_nul_and_noncharacters_are_valid() {
    for point in [
        0, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000, 0xfdd0, 0xfeff, 0xffff, 0x10000, 0x10ffff,
    ] {
        let mut buffer = [0; 4];
        let bytes = char::from_u32(point)
            .unwrap()
            .encode_utf8(&mut buffer)
            .as_bytes();
        assert!(wp_is_valid_utf8_bytes(bytes));
        assert_eq!(wp_scrub_utf8_bytes(bytes), bytes);
    }
}

#[test]
fn scrub_replaces_maximal_subparts_without_merging_adjacent_errors() {
    for (input, expected) in [
        (b"a\xe2\x8cZ".as_slice(), "a�Z"),
        (b"\xe2\x8c", "�"),
        (b"\xc0\xaf", "��"),
        (b"\xed\xa0\x80", "���"),
        (b"\xf4\x90\x80\x80", "����"),
        (b"\x80\xff\xf0\x90\x80", "���"),
    ] {
        assert!(!wp_is_valid_utf8_bytes(input));
        assert_eq!(wp_scrub_utf8_bytes(input), expected.as_bytes());
    }
}

#[test]
fn noncharacter_matching_is_independent_of_surrounding_invalid_bytes() {
    for point in (0xfdd0..=0xfdef)
        .chain((0..=16).flat_map(|plane| [plane * 65536 + 65534, plane * 65536 + 65535]))
    {
        let mut buffer = [0; 4];
        let mut bytes = vec![255];
        bytes.extend_from_slice(
            char::from_u32(point)
                .unwrap()
                .encode_utf8(&mut buffer)
                .as_bytes(),
        );
        bytes.push(255);
        assert!(!wp_is_valid_utf8_bytes(&bytes));
        assert!(wp_has_noncharacters_bytes(&bytes));
    }
    for bytes in [
        b"".as_slice(),
        b"\xef\xb7\x8f",
        b"\xef\xb7\xb0",
        b"\xf4\x9f\xbf\xbf",
        b"\xed\xbf\xbe",
    ] {
        assert!(!wp_has_noncharacters_bytes(bytes));
    }
}

use recastpress_wp::is_utf8_charset_bytes;

#[test]
fn both_aliases_accept_every_ascii_case_variant_without_changing_input() {
    for alias in [b"UTF8".as_slice(), b"UTF-8".as_slice()] {
        for case in 0..8 {
            let mut input = alias.to_vec();
            for (index, byte) in input[..3].iter_mut().enumerate() {
                if case & (1 << index) != 0 {
                    byte.make_ascii_lowercase();
                }
            }
            let original = input.clone();
            assert!(is_utf8_charset_bytes(&input), "{input:?}");
            assert_eq!(input, original);
        }
    }
}

#[test]
fn aliases_are_exact_byte_strings_without_trim_unicode_or_nul_shortcuts() {
    let rejected: &[&[u8]] = &[
        b"",
        b"UTF",
        b"UTF-",
        b"UTF_8",
        b"UTF--8",
        b"UTF-08",
        b"UTF-80",
        b"ASCII",
        b"Windows-1252",
        b" UTF-8",
        b"UTF8 ",
        b"\tUTF8",
        b"UTF-8\r\n",
        b"UTF8\0",
        b"UTF\0-8",
        b"\0UTF8",
        b"UTF8\xff",
        b"\xffUTF-8",
        "ＵＴＦ８".as_bytes(),
        "UTF−8".as_bytes(),
        "UTF‑8".as_bytes(),
        "\u{feff}UTF-8".as_bytes(),
        "UTF8\u{a0}".as_bytes(),
    ];
    for &input in rejected {
        let original = input.to_vec();
        assert!(!is_utf8_charset_bytes(input), "{input:?}");
        assert_eq!(input, original);
    }
}

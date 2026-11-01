use recastpress_wp::wp_is_json_media_type_c_bytes;

#[test]
fn unanchored_ascii_boundaries_do_not_require_a_whole_header_match() {
    for input in [
        b"application/json".as_slice(),
        b"text/plain,application/json;anything",
        b"arbitrary text application/json broken",
        b"application/json\ntrailing",
        b"\t\r\n\x0b\x0c application/json\x0cignored",
    ] {
        assert!(wp_is_json_media_type_c_bytes(input), "{input:?}");
    }
    for input in [
        b"".as_slice(),
        b"xapplication/json",
        b"text/application/json",
        b";application/json",
        b"application/jsonx",
        b"application/json/",
        b"application/json\\n",
    ] {
        assert!(!wp_is_json_media_type_c_bytes(input), "{input:?}");
    }
}

#[test]
fn source_range_accepts_non_mime_punctuation_and_requires_nonempty_prefix() {
    for input in [
        b"application/!#$&'()*+,-./:;<=>?@[\\]^_aZ+json".as_slice(),
        b"application//+json",
        b"application/a/b+json",
        b"application/a,b+json",
        b"application/a;b+json",
        b"application/++json",
        b"application/x,application/+json",
    ] {
        assert!(wp_is_json_media_type_c_bytes(input), "{input:?}");
    }
    for input in [
        b"application/+json".as_slice(),
        b"application/%+json",
        b"application/`+json",
        b"application/{+json",
        b"application/a +json",
    ] {
        assert!(!wp_is_json_media_type_c_bytes(input), "{input:?}");
    }
}

#[test]
fn case_oembed_nul_and_invalid_utf8_use_exact_byte_boundaries() {
    for (input, expected) in [
        (b"APPLICATION/JSON+OEMBED".as_slice(), true),
        (b"Application/vnd.Example+JsOn+OeMbEd;q=0", true),
        (b"application/json+oembedX", false),
        (b"application/json+oembed+json", true),
        (b"application/json\0", false),
        (b"\0application/json", false),
        (b"application/\xff+json", false),
        (b"\xff application/json ", true),
        (b"\xff application/json\xff", false),
        ("\u{a0}application/json".as_bytes(), false),
    ] {
        let original = input.to_vec();
        assert_eq!(wp_is_json_media_type_c_bytes(input), expected, "{input:?}");
        assert_eq!(input, original);
    }
}

#[test]
fn repeated_prefix_candidates_preserve_earliest_viable_content_start() {
    let prefix = b"application/x,".repeat(4096);
    let mut no_match = prefix.clone();
    no_match.extend_from_slice(b"application/jsonx");
    assert!(!wp_is_json_media_type_c_bytes(&no_match));
    let mut earlier_prefix_match = prefix;
    earlier_prefix_match.extend_from_slice(b"application/+json+oembed;");
    assert!(wp_is_json_media_type_c_bytes(&earlier_prefix_match));
    let mut broken_run = b"application/x,".repeat(4096);
    broken_run.extend_from_slice(b"\0application/+json");
    assert!(!wp_is_json_media_type_c_bytes(&broken_run));
}

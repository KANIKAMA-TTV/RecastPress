use recastpress_wp::{
    PreparedTitlePost, TexturizeLimits, TitleFormatError as Error,
    get_the_title_prepared_frontend_default_english_no_html_bytes as format_title,
};

fn convert(raw: &[u8]) -> Vec<u8> {
    let view = PreparedTitlePost::new(1, raw, b"", b"publish").unwrap();
    format_title(
        &view,
        TexturizeLimits::new(usize::MAX, usize::MAX, usize::MAX),
    )
    .unwrap()
}

#[test]
fn standard_title_callback_order_composes_quotes_entities_trim_and_title_context_capital_p() {
    let raw = b" \t\"Wordpress\" &a; &ab; &abcdefghi; ... 9x9 \r\n";
    assert_eq!(
        convert(raw),
        b"&#8220;WordPress&#8221; &#038;a; &ab; &#038;abcdefghi; &#8230; 9&#215;9"
    );
    assert_eq!(
        convert(b"Wordpress Wordpressing WORDPRESS wordpress WordPress"),
        b"WordPress WordPressing WORDPRESS wordpress WordPress"
    );
    assert_eq!(
        convert(b"'Wordpress' --"),
        b"&#8216;WordPress&#8217; &#8212;"
    );
    // convert_chars is not texturize's entity guard: one-character names change.
    assert_eq!(
        convert(b"&a; &1; &14; &abcdefgh; &abcdefghi;"),
        b"&#038;a; &#038;1; &14; &abcdefgh; &#038;abcdefghi;"
    );
    assert_eq!(
        convert(b"&&amp; &&a; && &#bad;"),
        b"&#038;&amp; &#038;&#038;a; &#038;&#038; &#038;#bad;"
    );
}

#[test]
fn php_trim_six_byte_set_preserves_form_feed_nbsp_entities_and_title_zero() {
    for (raw, expected) in [
        (b"".as_slice(), b"".as_slice()),
        (b"0", b"0"),
        (b"\0\t\r\n \x0b", b""),
        (b"\0 \x0b0\r\n\t", b"0"),
        (b"\x0cWordpress\x0c", b"\x0cWordPress\x0c"),
        (b"\xc2\xa0Wordpress\xc2\xa0", b"\xc2\xa0WordPress\xc2\xa0"),
        (b"&nbsp;Wordpress&nbsp;", b"&nbsp;WordPress&nbsp;"),
        (b"\0\xef\xbb\xbfWordpress\0", b"\xef\xbb\xbfWordPress"),
    ] {
        assert_eq!(convert(raw), expected);
    }
}

#[test]
fn constructor_requires_positive_publish_unprotected_profile_and_defers_title_admission() {
    for id in [i64::MIN, -1, 0] {
        assert!(matches!(
            PreparedTitlePost::new(id, b"title", b"", b"publish"),
            Err(Error::UnsupportedValue)
        ));
    }
    for status in [
        b"".as_slice(),
        b"private",
        b"draft",
        b"future",
        b"Publish",
        b"publish ",
        b"publish\0",
    ] {
        assert!(matches!(
            PreparedTitlePost::new(1, b"title", b"", status),
            Err(Error::UnsupportedValue)
        ));
    }
    for password in [b"00".as_slice(), b" ", b"\0", b"false", b"PRIVATE"] {
        assert!(matches!(
            PreparedTitlePost::new(1, b"title", password, b"publish"),
            Err(Error::UnsupportedValue)
        ));
    }
    for password in [b"".as_slice(), b"0"] {
        let view = PreparedTitlePost::new(i64::MAX, b"Wordpress", password, b"publish").unwrap();
        assert_eq!(
            format_title(&view, TexturizeLimits::new(9, 9, 9)).unwrap(),
            b"WordPress"
        );
    }
    for title in [b"<PRIVATE>".as_slice(), b"valid then\xff"] {
        let view = PreparedTitlePost::new(1, title, b"", b"publish").unwrap();
        assert_eq!(
            format_title(&view, TexturizeLimits::new(0, 0, 0)),
            Err(Error::InputLimit)
        );
        assert_eq!(
            format_title(
                &view,
                TexturizeLimits::new(title.len(), usize::MAX, usize::MAX)
            ),
            Err(Error::UnsupportedValue)
        );
    }
}

#[test]
fn output_cap_is_after_trim_and_later_convert_expansion_obeys_the_work_cap() {
    let raw = b" Wordpress ";
    let view = PreparedTitlePost::new(1, raw, b"", b"publish").unwrap();
    assert_eq!(
        format_title(&view, TexturizeLimits::new(raw.len(), raw.len(), 9)).unwrap(),
        b"WordPress"
    );
    for (limits, error) in [
        (
            TexturizeLimits::new(raw.len() - 1, raw.len(), 9),
            Error::InputLimit,
        ),
        (
            TexturizeLimits::new(raw.len(), raw.len() - 1, 9),
            Error::WorkLimit,
        ),
        (
            TexturizeLimits::new(raw.len(), raw.len(), 8),
            Error::OutputLimit,
        ),
    ] {
        assert_eq!(format_title(&view, limits), Err(error));
    }
    let empty = PreparedTitlePost::new(1, b"\t \0", b"0", b"publish").unwrap();
    assert_eq!(
        format_title(&empty, TexturizeLimits::new(3, 3, 0)),
        Ok(Vec::new())
    );
    let amp = PreparedTitlePost::new(1, b"&a;", b"", b"publish").unwrap();
    assert_eq!(
        format_title(&amp, TexturizeLimits::new(3, 8, 8)).unwrap(),
        b"&#038;a;"
    );
    assert_eq!(
        format_title(&amp, TexturizeLimits::new(3, 7, 8)),
        Err(Error::WorkLimit)
    );
    assert_eq!(
        format_title(&amp, TexturizeLimits::new(3, 8, 7)),
        Err(Error::OutputLimit)
    );
    let quote = PreparedTitlePost::new(1, b"'", b"", b"publish").unwrap();
    assert_eq!(
        format_title(&quote, TexturizeLimits::new(1, 7, 7)),
        Err(Error::WorkLimit)
    );
    assert_eq!(
        format_title(&quote, TexturizeLimits::new(1, 10, 7)).unwrap(),
        b"&#8216;"
    );
    let zero = PreparedTitlePost::new(1, b"0", b"", b"publish").unwrap();
    assert_eq!(
        format_title(&zero, TexturizeLimits::new(1, 1, 0)),
        Err(Error::OutputLimit)
    );
}

#[test]
fn raw_input_is_unchanged_and_result_remains_wp_bytes_rather_than_escaped_html_or_literal_title() {
    let raw = "\t日本語 🦀 > &copy; \"Wordpress\" mid\0line\rnext\t".as_bytes();
    let before = raw.to_vec();
    let result = convert(raw);
    assert_eq!(
        result,
        "日本語 🦀 > &copy; &#8220;WordPress&#8221; mid\0line\rnext".as_bytes()
    );
    assert_eq!(raw, before);
    // No esc_html/UTF-8-to-HTML policy or PreparedPosts literal conversion occurs.
    assert!(result.contains(&b'>'));
    assert!(result.windows(6).any(|part| part == b"&copy;"));
    assert!(result.contains(&0));
}

#[test]
fn view_and_errors_are_redacted_and_successful_output_outlives_the_borrowed_post() {
    let result = {
        let raw = b" PRIVATE Wordpress ".to_vec();
        let view = PreparedTitlePost::new(123, &raw, b"0", b"publish").unwrap();
        assert_eq!(format!("{view:?}"), "PreparedTitlePost([redacted])");
        format_title(
            &view,
            TexturizeLimits::new(usize::MAX, usize::MAX, usize::MAX),
        )
        .unwrap()
    };
    assert_eq!(result, b"PRIVATE WordPress");
    for error in [
        Error::UnsupportedValue,
        Error::InputLimit,
        Error::WorkLimit,
        Error::OutputLimit,
        Error::AllocationFailed,
    ] {
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

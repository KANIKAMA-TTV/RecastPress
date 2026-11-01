use recastpress_server::feeds::{
    FeedDocument, FeedEntry, FeedError, FeedFormat, UtcTimestamp, render,
};

fn timestamp() -> UtcTimestamp {
    UtcTimestamp::new(2026, 10, 26, 12, 34, 56).unwrap()
}
fn document() -> FeedDocument {
    FeedDocument {
        id: "https://example.test/feed-id".into(),
        title: "Public updates".into(),
        home_url: "https://example.test/".into(),
        self_url: "https://example.test/feed?x=1&y=2".into(),
        description: "Published articles".into(),
        author_name: "Editor".into(),
        updated: timestamp(),
        entries: vec![],
    }
}
fn entry(id: &str) -> FeedEntry {
    FeedEntry {
        id: id.into(),
        title: "Article".into(),
        link: "https://example.test/article".into(),
        summary_html: "<p>Public summary</p>".into(),
        author_name: "Writer".into(),
        published: UtcTimestamp::new(2000, 2, 29, 0, 0, 0).unwrap(),
        updated: timestamp(),
    }
}
fn xml(document: &FeedDocument, format: FeedFormat) -> String {
    String::from_utf8(render(document, format, usize::MAX).unwrap()).unwrap()
}

#[test]
fn timestamps_validate_gregorian_boundaries_and_format_utc_without_locale() {
    for args in [
        (1899, 1, 1, 0, 0, 0),
        (10000, 1, 1, 0, 0, 0),
        (1900, 2, 29, 0, 0, 0),
        (2100, 2, 29, 0, 0, 0),
        (2026, 0, 1, 0, 0, 0),
        (2026, 4, 31, 0, 0, 0),
        (2026, 1, 0, 0, 0, 0),
        (2026, 1, 1, 24, 0, 0),
        (2026, 1, 1, 0, 60, 0),
        (2026, 1, 1, 0, 0, 60),
    ] {
        assert_eq!(
            UtcTimestamp::new(args.0, args.1, args.2, args.3, args.4, args.5),
            Err(FeedError::InvalidTimestamp)
        );
    }
    for (date, atom, rss) in [
        (
            UtcTimestamp::new(1900, 1, 1, 0, 0, 0).unwrap(),
            "1900-01-01T00:00:00Z",
            "Mon, 01 Jan 1900 00:00:00 GMT",
        ),
        (
            UtcTimestamp::new(1970, 1, 1, 0, 0, 0).unwrap(),
            "1970-01-01T00:00:00Z",
            "Thu, 01 Jan 1970 00:00:00 GMT",
        ),
        (
            UtcTimestamp::new(2000, 2, 29, 0, 0, 0).unwrap(),
            "2000-02-29T00:00:00Z",
            "Tue, 29 Feb 2000 00:00:00 GMT",
        ),
        (
            timestamp(),
            "2026-10-26T12:34:56Z",
            "Mon, 26 Oct 2026 12:34:56 GMT",
        ),
        (
            UtcTimestamp::new(9999, 12, 31, 23, 59, 59).unwrap(),
            "9999-12-31T23:59:59Z",
            "Fri, 31 Dec 9999 23:59:59 GMT",
        ),
    ] {
        assert_eq!(date.atom_text(), atom);
        assert_eq!(date.rss_text(), rss);
    }
}

#[test]
fn empty_and_multiple_entry_feeds_have_required_elements_and_fixed_content_types() {
    let mut feed = document();
    let rss = xml(&feed, FeedFormat::Rss2);
    assert!(rss.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\""));
    assert!(rss.contains("<title>Public updates</title>\n<link>https://example.test/</link>\n<description>Published articles</description>"));
    assert!(rss.contains("xmlns:dc=\"http://purl.org/dc/elements/1.1/\""));
    assert!(!rss.contains("<item>"));
    let atom = xml(&feed, FeedFormat::Atom1);
    assert!(atom.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\">"));
    assert!(atom.contains("<id>https://example.test/feed-id</id>"));
    assert!(atom.contains("<author><name>Editor</name>\n</author>"));
    assert!(atom.contains("rel=\"self\""));
    assert!(atom.contains("rel=\"alternate\""));
    for id in ["https://example.test/one", "https://example.test/two"] {
        feed.entries.push(entry(id));
    }
    let rss = xml(&feed, FeedFormat::Rss2);
    let atom = xml(&feed, FeedFormat::Atom1);
    assert_eq!(rss.matches("<item>").count(), 2);
    assert_eq!(atom.matches("<entry>").count(), 2);
    assert!(rss.contains("<guid isPermaLink=\"false\">https://example.test/one</guid>"));
    assert!(rss.contains("<pubDate>Tue, 29 Feb 2000 00:00:00 GMT</pubDate>"));
    assert!(atom.contains("<published>2000-02-29T00:00:00Z</published>"));
    assert_eq!(
        FeedFormat::Rss2.content_type(),
        "application/rss+xml; charset=utf-8"
    );
    assert_eq!(
        FeedFormat::Atom1.content_type(),
        "application/atom+xml; charset=utf-8"
    );
}

#[test]
fn feeds_escape_text_and_prepared_html_as_data_without_cdata_or_dtd() {
    let mut feed = document();
    feed.title = "日&<>\"'\r\n\t".into();
    let mut item = entry("https://example.test/one");
    item.summary_html = "<p a=\"x&y\">日]]></p>\r\n".into();
    feed.entries.push(item);
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let output = xml(&feed, format);
        assert!(output.contains("日&amp;&lt;&gt;&quot;&apos;&#13;\n\t"));
        assert!(output.contains("&lt;p a=&quot;x&amp;y&quot;&gt;日]]&gt;&lt;/p&gt;&#13;\n"));
        assert!(output.contains("href=\"https://example.test/feed?x=1&amp;y=2\""));
        assert!(!output.contains("<![CDATA["));
        assert!(!output.contains("<!DOCTYPE"));
        assert!(!output.contains("<p a="));
    }
    assert!(xml(&feed, FeedFormat::Atom1).contains("<summary type=\"html\">"));
}

#[test]
fn feeds_reject_xml_forbidden_characters_and_retain_other_xml_characters() {
    for forbidden in ['\0', '\u{b}', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
        let mut feed = document();
        let mut item = entry("https://example.test/one");
        item.summary_html = format!("x{forbidden}y");
        feed.entries.push(item);
        for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
            assert_eq!(
                render(&feed, format, usize::MAX),
                Err(FeedError::InvalidXmlCharacter("entry.summary_html"))
            );
        }
        feed.entries.clear();
        feed.author_name = format!("x{forbidden}");
        assert_eq!(
            render(&feed, FeedFormat::Atom1, usize::MAX),
            Err(FeedError::InvalidXmlCharacter("feed.author_name"))
        );
    }
    let mut feed = document();
    feed.description = "\t\n\r\u{20}\u{d7ff}\u{e000}\u{fffd}\u{10000}\u{1fffe}\u{10ffff}".into();
    assert!(render(&feed, FeedFormat::Atom1, usize::MAX).is_ok());
}

#[test]
fn feed_validation_is_a_narrow_url_and_metadata_subset_with_unique_entry_ids() {
    for invalid in [
        "/relative",
        "//example.test/a",
        "urn:example:one",
        "ftp://example.test/a",
        "https://",
        "https://user@example.test/a",
        "https://例.test/a",
        "https://example.test/ bad",
        "https://example.test/\n",
        "https://example.test/\u{7f}",
        "https://example.test/%zz",
        "https://example.test/?x=%",
        "https://example.test/#%a",
        "https://example.test/#one#two",
        "https://example.test:abc/a",
        "https://example.test:65536/a",
        "https://[bad]/a",
        "https://[::1]extra/a",
        "https://example.test/a\\b",
        "https://example.test/a\"b",
        "https://example.test/[x]",
    ] {
        let mut feed = document();
        feed.id = invalid.into();
        assert!(
            render(&feed, FeedFormat::Atom1, usize::MAX).is_err(),
            "{invalid:?}"
        );
        let mut feed = document();
        feed.self_url = invalid.into();
        assert!(render(&feed, FeedFormat::Rss2, usize::MAX).is_err());
    }
    for valid in [
        "https://example.test/a%20b?x=%23#fragment?x=/a",
        "https://[::1]:8080/path",
        "https://example.test/?x='quoted'",
    ] {
        let mut feed = document();
        feed.id = valid.into();
        let output = xml(&feed, FeedFormat::Atom1);
        assert!(output.contains("<id>"));
        if valid.contains("#fragment") {
            assert!(output.contains("#fragment?x=/a</id>"));
        }
    }
    for field in ["title", "description", "author"] {
        let mut feed = document();
        match field {
            "title" => feed.title.clear(),
            "description" => feed.description.clear(),
            _ => feed.author_name = " \t".into(),
        }
        assert!(matches!(
            render(&feed, FeedFormat::Atom1, usize::MAX),
            Err(FeedError::MissingField(_))
        ));
    }
    let mut feed = document();
    feed.entries = vec![
        entry("https://example.test/same"),
        entry("https://example.test/same"),
    ];
    assert_eq!(
        render(&feed, FeedFormat::Rss2, usize::MAX),
        Err(FeedError::DuplicateEntryId)
    );
    // Candidate uniqueness is literal, not resource identity after normalization.
    feed.entries[1].id.push('#');
    assert!(render(&feed, FeedFormat::Atom1, usize::MAX).is_ok());
}

#[test]
fn output_cap_includes_escape_expansion_and_never_returns_a_partial_document() {
    let mut feed = document();
    feed.description = "&".repeat(100);
    feed.entries.push(entry("https://example.test/one"));
    let before = feed.clone();
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let complete = render(&feed, format, usize::MAX).unwrap();
        assert_eq!(render(&feed, format, complete.len()).unwrap(), complete);
        for cap in [0, 1, complete.len() / 2, complete.len() - 1] {
            assert_eq!(
                render(&feed, format, cap),
                Err(FeedError::OutputLimitExceeded)
            );
        }
    }
    assert_eq!(feed, before);
}

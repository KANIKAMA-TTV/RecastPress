use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use recastpress_server::feeds::comments::{
    CommentFeedDocument, CommentFeedEntry, CommentReply, render, render_budgeted,
};
use recastpress_server::feeds::{
    BudgetedFeedError, BudgetedFeedOutput, FeedEntry, FeedError, FeedFormat, UtcTimestamp,
};
use std::sync::Arc;

fn document() -> CommentFeedDocument {
    let time = UtcTimestamp::new(2000, 2, 29, 12, 34, 56).unwrap();
    CommentFeedDocument {
        id: "https://example.test/comments-id".into(),
        title: "Comments & <title>".into(),
        home_url: "https://example.test/post".into(),
        self_url: "https://example.test/comments".into(),
        description: "Public comments".into(),
        author_name: "Feed author".into(),
        updated: time,
        entries: vec![CommentFeedEntry {
            entry: FeedEntry {
                id: "https://example.test/post-id#comment-1".into(),
                title: "By: A & B".into(),
                link: "https://example.test/post?x=1&y=2#comment-1".into(),
                summary_html: "<p>Short & prepared</p>\r\n".into(),
                author_name: "A & B".into(),
                published: time,
                updated: time,
            },
            content_html: "<p>Full 日本語 & <a href=\"relative\">link</a>]]></p>\r\n".into(),
            reply_to: CommentReply {
                reference: "https://example.test/absent-parent-id#comment-999".into(),
                href: "https://example.test/post?x=3&y=4#comment-999".into(),
            },
        }],
    }
}

#[test]
fn rss_keeps_description_separate_from_full_content_without_cdata_or_input_mutation() {
    let doc = document();
    let saved = doc.clone();
    let rss = String::from_utf8(render(&doc, FeedFormat::Rss2, 65536).unwrap()).unwrap();
    assert!(rss.contains("xmlns:content=\"http://purl.org/rss/1.0/modules/content/\""));
    assert!(
        rss.contains("<description>&lt;p&gt;Short &amp; prepared&lt;/p&gt;&#13;\n</description>")
    );
    assert!(rss.contains("<content:encoded>&lt;p&gt;Full 日本語 &amp; &lt;a href=&quot;relative&quot;&gt;link&lt;/a&gt;]]&gt;&lt;/p&gt;&#13;\n</content:encoded>"));
    assert!(
        rss.contains("<guid isPermaLink=\"false\">https://example.test/post-id#comment-1</guid>")
    );
    assert!(rss.contains("<pubDate>Tue, 29 Feb 2000 12:34:56 GMT</pubDate>"));
    assert!(!rss.contains("<![CDATA["));
    assert!(!rss.contains("<!DOCTYPE"));
    assert_eq!(doc, saved);
}

#[test]
fn atom_full_content_has_explicit_base_and_parent_reference_without_parent_membership() {
    let doc = document();
    let atom = String::from_utf8(render(&doc, FeedFormat::Atom1, 65536).unwrap()).unwrap();
    assert!(atom.contains(
        "<content type=\"html\" xml:base=\"https://example.test/post?x=1&amp;y=2#comment-1\">"
    ));
    assert!(atom.contains("<thr:in-reply-to ref=\"https://example.test/absent-parent-id#comment-999\" href=\"https://example.test/post?x=3&amp;y=4#comment-999\" type=\"text/html\"/>"));
    assert!(atom.contains("<published>2000-02-29T12:34:56Z</published>"));
    assert!(!atom.contains("<summary"));
    assert!(!atom.contains("Short &amp; prepared"));
    let mut empty = doc;
    empty.entries.clear();
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let xml = String::from_utf8(render(&empty, format, 65536).unwrap()).unwrap();
        assert!(xml.contains("<title>Comments &amp; &lt;title&gt;</title>"));
        assert!(!xml.contains("<item>"));
        assert!(!xml.contains("<entry>"));
    }
}

#[test]
fn all_fields_validate_before_caps_and_pool_including_rss_unused_reply_and_atom_description() {
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let pool = BudgetPool::new(0);
        let mut doc = document();
        doc.entries[0].reply_to.href = "https://example.test/%zz".into();
        assert!(matches!(
            render_budgeted(&doc, format, 0, pool.clone()),
            Err(BudgetedFeedError::Feed(FeedError::InvalidUrl(
                "comment.reply_to.href"
            )))
        ));
        doc = document();
        doc.entries[0].entry.summary_html.push('\0');
        assert!(matches!(
            render_budgeted(&doc, format, 0, pool.clone()),
            Err(BudgetedFeedError::Feed(FeedError::InvalidXmlCharacter(
                "entry.summary_html"
            )))
        ));
        doc = document();
        doc.entries[0].content_html.push('\u{ffff}');
        assert!(matches!(
            render_budgeted(&doc, format, 0, pool.clone()),
            Err(BudgetedFeedError::Feed(FeedError::InvalidXmlCharacter(
                "comment.content_html"
            )))
        ));
        doc = document();
        doc.entries.push(doc.entries[0].clone());
        assert!(matches!(
            render_budgeted(&doc, format, 0, pool.clone()),
            Err(BudgetedFeedError::Feed(FeedError::DuplicateEntryId))
        ));
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

#[test]
fn escaped_final_cap_and_exact_pool_charge_match_plain_output_with_failure_release() {
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let doc = document();
        let expected = render(&doc, format, 65536).unwrap();
        let charge = expected.len() + size_of::<BudgetedFeedOutput>();
        assert_eq!(render(&doc, format, expected.len()).unwrap(), expected);
        assert_eq!(
            render(&doc, format, expected.len() - 1),
            Err(FeedError::OutputLimitExceeded)
        );
        for limit in [0, charge - 1] {
            let pool = BudgetPool::new(limit);
            assert!(matches!(
                render_budgeted(&doc, format, expected.len(), pool.clone()),
                Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
            ));
            assert_eq!(pool.stats().unwrap().claimed, 0);
        }
        let pool = BudgetPool::new(charge);
        assert!(matches!(
            render_budgeted(&doc, format, expected.len() - 1, pool.clone()),
            Err(BudgetedFeedError::Feed(FeedError::OutputLimitExceeded))
        ));
        assert_eq!(pool.stats().unwrap().claimed, 0);
        let output = render_budgeted(&doc, format, expected.len(), pool.clone()).unwrap();
        assert_eq!(output.as_bytes(), expected);
        assert_eq!(output.format(), format);
        assert_eq!(output.charged_bytes(), charge);
        drop(output);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

#[test]
fn shared_output_last_drop_old_plus_new_and_other_kind_debt_use_the_same_pool() {
    let doc = document();
    let format = FeedFormat::Atom1;
    let charge = render(&doc, format, 65536).unwrap().len() + size_of::<BudgetedFeedOutput>();
    let pool = BudgetPool::new(2 * charge + 7);
    let other = pool.try_reserve(ResourceKind::ResponseCache, 7).unwrap();
    let old = Arc::new(render_budgeted(&doc, format, 65536, pool.clone()).unwrap());
    let retained = Arc::clone(&old);
    drop(old);
    assert_eq!(pool.stats().unwrap().claimed, charge + 7);
    let new = render_budgeted(&doc, format, 65536, pool.clone()).unwrap();
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        2 * charge
    );
    pool.set_limit(0).unwrap();
    assert!(!retained.as_bytes().is_empty());
    assert!(matches!(
        render_budgeted(&doc, format, 65536, pool.clone()),
        Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
    ));
    drop(new);
    assert_eq!(pool.stats().unwrap().claimed, charge + 7);
    drop(retained);
    assert_eq!(pool.stats().unwrap().claimed, 7);
    drop(other);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

//! Fixed public-data serializer fixture only: no query, DB or endpoint.
use recastpress_core::resource_budget::{BudgetPool, ResourceKind};
use recastpress_server::feeds::comments::{
    CommentFeedDocument, CommentFeedEntry, CommentReply, render, render_budgeted,
};
use recastpress_server::feeds::{BudgetedFeedOutput, FeedEntry, FeedFormat, UtcTimestamp};
use serde_json::{Value, json};
use std::sync::Arc;

fn observed(
    document: &CommentFeedDocument,
    format: FeedFormat,
    label: &str,
    observations: &mut Vec<Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let expected = render(document, format, 65536)?;
    let pool = BudgetPool::new(1024 * 1024);
    let output = Arc::new(render_budgeted(
        document,
        format,
        expected.len(),
        pool.clone(),
    )?);
    assert_eq!(output.as_bytes(), expected);
    assert_eq!(output.format(), format);
    let charged = output.charged_bytes();
    let rendering = pool
        .stats()
        .expect("healthy fixture pool")
        .claimed_for(ResourceKind::Rendering);
    let retained = Arc::clone(&output);
    drop(output);
    let after_first = pool.stats().expect("healthy fixture pool").claimed;
    // The returned observation String is an explicit unpooled fixture copy.
    let xml = String::from_utf8(retained.as_bytes().to_vec())?;
    drop(retained);
    observations.push(json!({"document": label,"output_bytes":xml.len(),"owner_bytes":size_of::<BudgetedFeedOutput>(),
        "charged_bytes":charged,"rendering_before_drop":rendering,"claimed_after_first_drop":after_first,
        "claimed_after_final_drop":pool.stats().expect("healthy fixture pool").claimed}));
    Ok(xml)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let time = UtcTimestamp::new(2000, 2, 29, 12, 34, 56)?;
    let mut document = CommentFeedDocument {
        id: "https://example.test/comments-id".into(),
        title: "Comments <公開> & readers".into(),
        home_url: "https://example.test/post".into(),
        self_url: "https://example.test/comments/rss".into(),
        description: "Public comments\r\n<&>".into(),
        author_name: "Feed author".into(),
        updated: time,
        entries: vec![
            CommentFeedEntry {
                entry: FeedEntry {
                    id: "https://example.test/post-id#comment-1".into(),
                    title: "By: A & B".into(),
                    link: "https://example.test/post?x=1&y=2#comment-1".into(),
                    summary_html: "<p>Summary & prepared</p>\r\n".into(),
                    author_name: "A & B".into(),
                    published: time,
                    updated: time,
                },
                content_html: "<p>Full 日本語 & <a href=\"relative\">link</a>]]></p>\r\n".into(),
                reply_to: CommentReply {
                    reference: "https://example.test/post-id".into(),
                    href: "https://example.test/post#comments".into(),
                },
            },
            CommentFeedEntry {
                entry: FeedEntry {
                    id: "https://example.test/post-id#comment-2".into(),
                    title: "By: Second".into(),
                    link: "https://example.test/post#comment-2".into(),
                    summary_html: String::new(),
                    author_name: "Second".into(),
                    published: UtcTimestamp::new(1970, 1, 1, 0, 0, 0)?,
                    updated: time,
                },
                content_html: String::new(),
                reply_to: CommentReply {
                    reference: "https://example.test/post-id#comment-999".into(),
                    href: "https://example.test/post?x=3&y=4#comment-999".into(),
                },
            },
        ],
    };
    let mut observations = Vec::new();
    let rss = observed(&document, FeedFormat::Rss2, "rss", &mut observations)?;
    document.self_url = "https://example.test/comments/atom".into();
    let atom = observed(&document, FeedFormat::Atom1, "atom", &mut observations)?;
    document.entries.clear();
    document.self_url = "https://example.test/comments/rss".into();
    let empty_rss = observed(&document, FeedFormat::Rss2, "empty_rss", &mut observations)?;
    document.self_url = "https://example.test/comments/atom".into();
    let empty_atom = observed(
        &document,
        FeedFormat::Atom1,
        "empty_atom",
        &mut observations,
    )?;
    println!(
        "{}",
        json!({"rss":rss,"atom":atom,"empty_rss":empty_rss,"empty_atom":empty_atom,
        "rss_content_type":FeedFormat::Rss2.content_type(),"atom_content_type":FeedFormat::Atom1.content_type(),
        "budgeted_output_observations":observations})
    );
    Ok(())
}

//! Comment XML serialization of caller-prepared public data, not comment selection.
//!
//! Fixed WP reference: feed-rss2-comments.php:102 and feed-atom-comments.php:109.
//! Atom threading: https://www.rfc-editor.org/rfc/rfc4685.html#section-3.
//! Replies are explicit references: no post/comment classification, parent lookup
//! or requirement that a referenced parent occur in this feed. Both formats
//! validate every supplied field. No private-data/HTML sanitizer, query/filter,
//! locale, DB, endpoint registration or continuous publication is implemented.

use super::{
    BudgetedFeedError, BudgetedFeedOutput, FeedEntry, FeedError, FeedFormat, UtcTimestamp,
    XmlWriter, build_budgeted_output, text, url,
};
use recastpress_core::resource_budget::BudgetPool;
use std::collections::BTreeSet;

/// Caller-prepared parent identifier and HTML representation link. Neither is
/// inferred or fetched. The same HTTP(S) ASCII subset as article feeds applies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentReply {
    pub reference: String,
    pub href: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentFeedEntry {
    /// summary_html supplies the RSS description, independently of full content.
    pub entry: FeedEntry,
    /// HTML already prepared by the caller for public syndication.
    pub content_html: String,
    /// Always explicit, including a top-level comment's parent resource.
    pub reply_to: CommentReply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentFeedDocument {
    pub id: String,
    pub title: String,
    pub home_url: String,
    pub self_url: String,
    pub description: String,
    pub author_name: String,
    pub updated: UtcTimestamp,
    pub entries: Vec<CommentFeedEntry>,
}

fn validate(document: &CommentFeedDocument) -> Result<(), FeedError> {
    url(&document.id, "feed.id")?;
    url(&document.home_url, "feed.home_url")?;
    url(&document.self_url, "feed.self_url")?;
    text(&document.title, "feed.title", true)?;
    text(&document.description, "feed.description", true)?;
    text(&document.author_name, "feed.author_name", true)?;
    let mut ids = BTreeSet::new();
    for comment in &document.entries {
        let entry = &comment.entry;
        url(&entry.id, "entry.id")?;
        url(&entry.link, "entry.link")?;
        text(&entry.title, "entry.title", true)?;
        text(&entry.author_name, "entry.author_name", true)?;
        text(&entry.summary_html, "entry.summary_html", false)?;
        text(&comment.content_html, "comment.content_html", false)?;
        url(&comment.reply_to.reference, "comment.reply_to.reference")?;
        url(&comment.reply_to.href, "comment.reply_to.href")?;
        if !ids.insert(entry.id.as_str()) {
            return Err(FeedError::DuplicateEntryId);
        }
    }
    Ok(())
}

/// Complete comment XML, with the same URI/XML/UTC subset as article feeds.
/// The final cap counts escaped output bytes, not inputs or validation work.
/// No CDATA is emitted: HTML is escaped XML text, without sanitizing the HTML.
pub fn render(
    document: &CommentFeedDocument,
    format: FeedFormat,
    max_bytes: usize,
) -> Result<Vec<u8>, FeedError> {
    validate(document)?;
    let mut counter = XmlWriter::new(max_bytes, false);
    render_xml(&mut counter, document, format)?;
    let mut output = XmlWriter::new(counter.length, true);
    output
        .bytes
        .try_reserve_exact(counter.length)
        .map_err(|_| FeedError::OutputLimitExceeded)?;
    render_xml(&mut output, document, format)?;
    Ok(output.bytes)
}

/// Validate, measure, reserve Rendering, allocate final bytes and emit with the
/// same traversal. The shared move-only output charges escaped length plus its
/// owner header; bytes drop before its lease. Arc sharing retains that lease.
/// Inputs, validation sets/URI work, timestamp strings, allocator rounding,
/// copies and Arc nodes are unpooled. This is not RSS or capacity policy.
pub fn render_budgeted(
    document: &CommentFeedDocument,
    format: FeedFormat,
    max_bytes: usize,
    pool: BudgetPool,
) -> Result<BudgetedFeedOutput, BudgetedFeedError> {
    validate(document).map_err(BudgetedFeedError::Feed)?;
    let mut counter = XmlWriter::new(max_bytes, false);
    render_xml(&mut counter, document, format).map_err(BudgetedFeedError::Feed)?;
    build_budgeted_output(format, counter.length, pool, |output| {
        render_xml(output, document, format)
    })
}

fn render_xml(
    output: &mut XmlWriter,
    document: &CommentFeedDocument,
    format: FeedFormat,
) -> Result<(), FeedError> {
    output.push("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    match format {
        FeedFormat::Rss2 => render_rss(output, document),
        FeedFormat::Atom1 => render_atom(output, document),
    }
}

fn render_rss(output: &mut XmlWriter, document: &CommentFeedDocument) -> Result<(), FeedError> {
    output.push("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"><channel>\n")?;
    output.element("title", &document.title)?;
    output.element("link", &document.home_url)?;
    output.element("description", &document.description)?;
    output.link(
        "atom:link",
        "self",
        &document.self_url,
        Some(FeedFormat::Rss2.content_type()),
    )?;
    output.element("dc:creator", &document.author_name)?;
    output.element("lastBuildDate", &document.updated.rss_text())?;
    for comment in &document.entries {
        let entry = &comment.entry;
        output.push("<item>\n")?;
        output.element("title", &entry.title)?;
        output.element("link", &entry.link)?;
        output.element("dc:creator", &entry.author_name)?;
        output.element("pubDate", &entry.published.rss_text())?;
        output.push("<guid isPermaLink=\"false\">")?;
        output.escaped(&entry.id, false)?;
        output.push("</guid>\n")?;
        output.element("description", &entry.summary_html)?;
        output.element("content:encoded", &comment.content_html)?;
        output.push("</item>\n")?;
    }
    output.push("</channel></rss>\n")
}

fn render_atom(output: &mut XmlWriter, document: &CommentFeedDocument) -> Result<(), FeedError> {
    output.push("<feed xmlns=\"http://www.w3.org/2005/Atom\" xmlns:thr=\"http://purl.org/syndication/thread/1.0\">\n")?;
    output.element("id", &document.id)?;
    output.element("title", &document.title)?;
    output.element("subtitle", &document.description)?;
    output.element("updated", &document.updated.atom_text())?;
    output.author(&document.author_name)?;
    output.link("link", "alternate", &document.home_url, Some("text/html"))?;
    output.link(
        "link",
        "self",
        &document.self_url,
        Some(FeedFormat::Atom1.content_type()),
    )?;
    for comment in &document.entries {
        let entry = &comment.entry;
        output.push("<entry>\n")?;
        output.element("title", &entry.title)?;
        output.link("link", "alternate", &entry.link, Some("text/html"))?;
        output.author(&entry.author_name)?;
        output.element("id", &entry.id)?;
        output.element("updated", &entry.updated.atom_text())?;
        output.element("published", &entry.published.atom_text())?;
        output.push("<content type=\"html\" xml:base=\"")?;
        output.escaped(&entry.link, true)?;
        output.push("\">")?;
        output.escaped(&comment.content_html, false)?;
        output.push("</content>\n<thr:in-reply-to ref=\"")?;
        output.escaped(&comment.reply_to.reference, true)?;
        output.push("\" href=\"")?;
        output.escaped(&comment.reply_to.href, true)?;
        output.push("\" type=\"text/html\"/>\n</entry>\n")?;
    }
    output.push("</feed>\n")
}

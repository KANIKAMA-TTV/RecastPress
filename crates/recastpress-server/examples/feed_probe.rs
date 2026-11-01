//! Local development fixture for independent XML checks. No network or DB.
use recastpress_core::resource_budget::{BudgetPool, ResourceKind};
use recastpress_server::feeds::{
    BudgetedFeedOutput, FeedDocument, FeedEntry, FeedFormat, UtcTimestamp, render, render_budgeted,
};
use serde_json::{Value, json};
use std::sync::Arc;

fn observed_render(
    label: &str,
    feed: &FeedDocument,
    format: FeedFormat,
    pool: &BudgetPool,
    observations: &mut Vec<Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    // The comparison copy and returned XML string are fixture observations,
    // outside the retained-output accounting under test.
    let expected = render(feed, format, 65536)?;
    let output = Arc::new(render_budgeted(feed, format, 65536, pool.clone())?);
    assert_eq!(output.as_bytes(), expected);
    assert_eq!(output.format(), format);
    let charged = output.charged_bytes();
    let before_drop = pool.stats().expect("healthy fixture pool");
    let retained = Arc::clone(&output);
    drop(output);
    let after_first_drop = pool.stats().expect("healthy fixture pool").claimed;
    let text = String::from_utf8(retained.as_bytes().to_vec())?;
    drop(retained);
    observations.push(json!({"document":label, "output_bytes":text.len(),
        "owner_bytes":std::mem::size_of::<BudgetedFeedOutput>(), "charged_bytes":charged,
        "claimed_before_drop":before_drop.claimed,
        "rendering_before_drop":before_drop.claimed_for(ResourceKind::Rendering),
        "claimed_after_first_drop":after_first_drop,
        "claimed_after_final_drop":pool.stats().expect("healthy fixture pool").claimed}));
    Ok(text)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = FeedDocument {
        id: "https://example.test/feed-id".into(),
        title: "Site & \"friends\" <研究>".into(),
        home_url: "https://example.test/".into(),
        self_url: "https://example.test/feed/rss".into(),
        description: "Description\rline\nnext\t<&>".into(),
        author_name: "Feed author & friends".into(),
        updated: UtcTimestamp::new(2026, 10, 26, 3, 4, 5)?,
        entries: vec![
            FeedEntry {
                id: "https://example.test/entry-id?x=1&y=2".into(),
                title: "Entry <title> & \"quote\"".into(),
                link: "https://example.test/entry?x=1&y=2".into(),
                summary_html: "<p>A & B &lt;em&gt;日本語</p>\r\n".into(),
                author_name: "Author ' & name".into(),
                published: UtcTimestamp::new(1970, 1, 1, 0, 0, 0)?,
                updated: UtcTimestamp::new(2026, 10, 26, 3, 4, 5)?,
            },
            FeedEntry {
                id: "https://example.test/leap-id".into(),
                title: "Leap-day entry".into(),
                link: "https://example.test/leap".into(),
                summary_html: String::new(),
                author_name: "Second author".into(),
                published: UtcTimestamp::new(2000, 2, 29, 12, 34, 56)?,
                updated: UtcTimestamp::new(2000, 2, 29, 12, 34, 56)?,
            },
        ],
    };
    let pool = BudgetPool::new(1024 * 1024);
    let mut observations = Vec::new();
    let rss = observed_render("rss", &feed, FeedFormat::Rss2, &pool, &mut observations)?;
    feed.self_url = "https://example.test/feed/atom".into();
    let atom = observed_render("atom", &feed, FeedFormat::Atom1, &pool, &mut observations)?;
    feed.entries.clear();
    feed.self_url = "https://example.test/feed/rss".into();
    let empty_rss = observed_render(
        "empty_rss",
        &feed,
        FeedFormat::Rss2,
        &pool,
        &mut observations,
    )?;
    feed.self_url = "https://example.test/feed/atom".into();
    let empty_atom = observed_render(
        "empty_atom",
        &feed,
        FeedFormat::Atom1,
        &pool,
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

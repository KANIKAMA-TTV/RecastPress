use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use recastpress_server::feeds::{
    BudgetedFeedError, BudgetedFeedOutput, FeedDocument, FeedEntry, FeedError, FeedFormat,
    UtcTimestamp, render, render_budgeted,
};
use std::sync::Arc;

fn document(format: FeedFormat) -> FeedDocument {
    let time = UtcTimestamp::new(2000, 2, 29, 12, 34, 56).unwrap();
    FeedDocument {
        id: "https://example.test/feed-id".into(),
        title: "A & <B> \"日\"\r".into(),
        home_url: "https://example.test/".into(),
        self_url: match format {
            FeedFormat::Rss2 => "https://example.test/feed/rss",
            FeedFormat::Atom1 => "https://example.test/feed/atom",
        }
        .into(),
        description: "Description\r\n\t&<>".into(),
        author_name: "Author ' & name".into(),
        updated: time,
        entries: vec![FeedEntry {
            id: "https://example.test/entry-1".into(),
            title: "Entry <title> & text".into(),
            link: "https://example.test/entry-1?x=1&y=2".into(),
            summary_html: "<p>Public & prepared</p>\r\n".into(),
            author_name: "Entry author".into(),
            published: time,
            updated: time,
        }],
    }
}

fn output_charge(bytes: &[u8]) -> usize {
    bytes.len() + std::mem::size_of::<BudgetedFeedOutput>()
}

fn rendering_claim(pool: &BudgetPool, expected: usize) {
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        expected
    );
}

#[test]
fn budgeted_rss_and_atom_are_byte_identical_with_complete_xml_and_owner_header_charged() {
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let doc = document(format);
        let snapshot = doc.clone();
        let expected = render(&doc, format, usize::MAX).unwrap();
        let charge = output_charge(&expected);
        let pool = BudgetPool::new(charge);
        let output = render_budgeted(&doc, format, expected.len(), pool.clone()).unwrap();
        assert_eq!(output.as_bytes(), expected);
        assert_eq!(output.format(), format);
        assert_eq!(output.charged_bytes(), charge);
        assert_eq!(pool.stats().unwrap().claimed, charge);
        rendering_claim(&pool, charge);
        assert!(String::from_utf8(expected).unwrap().contains("&amp;"));
        assert!(!format!("{output:?}").contains("Public & prepared"));
        assert_eq!(doc, snapshot);
        drop(output);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

#[test]
fn output_cap_counts_escaped_xml_without_owner_header_and_rejects_one_byte_short() {
    for format in [FeedFormat::Rss2, FeedFormat::Atom1] {
        let doc = document(format);
        let expected = render(&doc, format, usize::MAX).unwrap();
        let pool = BudgetPool::new(usize::MAX);
        for cap in [0, expected.len() - 1] {
            assert!(matches!(
                render_budgeted(&doc, format, cap, pool.clone()),
                Err(BudgetedFeedError::Feed(FeedError::OutputLimitExceeded))
            ));
            assert_eq!(pool.stats().unwrap().claimed, 0);
        }
        let output = render_budgeted(&doc, format, expected.len(), pool.clone()).unwrap();
        assert_eq!(output.as_bytes().len(), expected.len());
        assert_eq!(output.charged_bytes(), output_charge(&expected));
        assert!(output.charged_bytes() > expected.len());
        drop(output);
        rendering_claim(&pool, 0);
    }
}

#[test]
fn zero_or_one_byte_short_pool_is_exact_budget_refusal_without_partial_claims() {
    let format = FeedFormat::Atom1;
    let doc = document(format);
    let expected = render(&doc, format, usize::MAX).unwrap();
    let charge = output_charge(&expected);
    for limit in [0, charge - 1] {
        let pool = BudgetPool::new(limit);
        assert!(matches!(
            render_budgeted(&doc, format, expected.len(), pool.clone()),
            Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
        ));
        rendering_claim(&pool, 0);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
    let error = BudgetedFeedError::AllocationFailed;
    let object: &dyn std::error::Error = &error;
    assert_eq!(object.to_string(), "feed output allocation failed");
    assert!(object.source().is_none());
}

#[test]
fn arc_sharing_retains_one_lease_and_last_drop_releases_after_bytes_remain_readable() {
    let format = FeedFormat::Rss2;
    let doc = document(format);
    let expected = render(&doc, format, usize::MAX).unwrap();
    let charge = output_charge(&expected);
    let pool = BudgetPool::new(charge);
    let first = Arc::new(render_budgeted(&doc, format, expected.len(), pool.clone()).unwrap());
    let second = Arc::clone(&first);
    let third = Arc::clone(&second);
    rendering_claim(&pool, charge);
    drop(first);
    drop(second);
    assert_eq!(third.as_bytes(), expected);
    assert_eq!(third.format(), format);
    rendering_claim(&pool, charge);
    drop(third);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn old_and_new_outputs_need_both_complete_charges_before_either_is_dropped() {
    let rss = document(FeedFormat::Rss2);
    let atom = document(FeedFormat::Atom1);
    let rss_bytes = render(&rss, FeedFormat::Rss2, usize::MAX).unwrap();
    let atom_bytes = render(&atom, FeedFormat::Atom1, usize::MAX).unwrap();
    let old_charge = output_charge(&rss_bytes);
    let new_charge = output_charge(&atom_bytes);
    let pool = BudgetPool::new(old_charge + new_charge - 1);
    let old = render_budgeted(&rss, FeedFormat::Rss2, usize::MAX, pool.clone()).unwrap();
    assert!(matches!(
        render_budgeted(&atom, FeedFormat::Atom1, usize::MAX, pool.clone()),
        Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
    ));
    assert_eq!(old.as_bytes(), rss_bytes);
    rendering_claim(&pool, old_charge);
    pool.set_limit(old_charge + new_charge).unwrap();
    let new = render_budgeted(&atom, FeedFormat::Atom1, usize::MAX, pool.clone()).unwrap();
    assert_eq!(new.as_bytes(), atom_bytes);
    rendering_claim(&pool, old_charge + new_charge);
    drop(old);
    rendering_claim(&pool, new_charge);
    drop(new);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn shrinking_pool_records_live_output_debt_without_mutating_or_revoking_bytes() {
    let doc = document(FeedFormat::Atom1);
    let expected = render(&doc, FeedFormat::Atom1, usize::MAX).unwrap();
    let charge = output_charge(&expected);
    let pool = BudgetPool::new(charge);
    let output = render_budgeted(&doc, FeedFormat::Atom1, usize::MAX, pool.clone()).unwrap();
    pool.set_limit(0).unwrap();
    assert_eq!(pool.stats().unwrap().debt, charge);
    assert_eq!(output.as_bytes(), expected);
    assert!(matches!(
        render_budgeted(&doc, FeedFormat::Atom1, usize::MAX, pool.clone()),
        Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
    ));
    rendering_claim(&pool, charge);
    drop(output);
    assert_eq!(pool.stats().unwrap().claimed, 0);
    assert_eq!(pool.stats().unwrap().debt, 0);
}

#[test]
fn rendering_competes_with_other_resource_kinds_and_releases_only_its_own_charge() {
    let doc = document(FeedFormat::Rss2);
    let expected = render(&doc, FeedFormat::Rss2, usize::MAX).unwrap();
    let charge = output_charge(&expected);
    let pool = BudgetPool::new(charge + 99);
    let other = pool.try_reserve(ResourceKind::InternalRows, 100).unwrap();
    assert!(matches!(
        render_budgeted(&doc, FeedFormat::Rss2, usize::MAX, pool.clone()),
        Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
    ));
    rendering_claim(&pool, 0);
    pool.set_limit(charge + 100).unwrap();
    let output = render_budgeted(&doc, FeedFormat::Rss2, usize::MAX, pool.clone()).unwrap();
    assert_eq!(pool.stats().unwrap().claimed, charge + 100);
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::InternalRows),
        100
    );
    drop(output);
    assert_eq!(pool.stats().unwrap().claimed, 100);
    rendering_claim(&pool, 0);
    drop(other);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn validation_precedes_output_cap_and_pool_admission_without_disturbing_live_outputs() {
    let doc = document(FeedFormat::Atom1);
    let pool = BudgetPool::new(usize::MAX);
    let old = render_budgeted(&doc, FeedFormat::Atom1, usize::MAX, pool.clone()).unwrap();
    let before = pool.stats().unwrap();
    let mut missing = doc.clone();
    missing.title.clear();
    let mut illegal = doc.clone();
    illegal.entries[0].summary_html.push('\u{1}');
    let mut duplicate = doc.clone();
    duplicate.entries.push(duplicate.entries[0].clone());
    for (invalid, error) in [
        (missing, FeedError::MissingField("feed.title")),
        (
            illegal,
            FeedError::InvalidXmlCharacter("entry.summary_html"),
        ),
        (duplicate, FeedError::DuplicateEntryId),
    ] {
        for selected_pool in [pool.clone(), BudgetPool::new(0)] {
            assert!(
                matches!(render_budgeted(&invalid, FeedFormat::Atom1, 0, selected_pool.clone()),
                Err(BudgetedFeedError::Feed(actual)) if actual == error)
            );
        }
        assert_eq!(pool.stats().unwrap(), before);
    }
    assert_eq!(
        old.as_bytes(),
        render(&doc, FeedFormat::Atom1, usize::MAX).unwrap()
    );
    drop(old);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

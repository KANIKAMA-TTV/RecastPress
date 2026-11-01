use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use recastpress_db::{
    PostMetaRow,
    budgeted_meta_view::{BudgetedPostMetaView, BudgetedViewError},
    meta_view::{ChangeError, LimitedChangeError, MetaChange, SearchPlan},
};
use std::sync::Arc;

fn rows() -> Vec<PostMetaRow> {
    vec![
        PostMetaRow {
            meta_id: 0,
            post_id: u64::MAX,
            meta_key: None,
            meta_value: Some(vec![]),
        },
        PostMetaRow {
            meta_id: 1,
            post_id: 0,
            meta_key: Some(vec![]),
            meta_value: None,
        },
        PostMetaRow {
            meta_id: u64::MAX,
            post_id: u64::MAX,
            meta_key: Some(b"\0\xff".to_vec()),
            meta_value: Some("語".as_bytes().to_vec()),
        },
    ]
}

fn row_bytes(rows: &[PostMetaRow]) -> usize {
    rows.iter()
        .map(|r| {
            std::mem::size_of::<PostMetaRow>()
                + r.meta_key.as_ref().map_or(0, Vec::len)
                + r.meta_value.as_ref().map_or(0, Vec::len)
        })
        .sum()
}

fn index_bytes() -> usize {
    3 * std::mem::size_of::<usize>() + 2 * std::mem::size_of::<(Vec<u8>, Vec<usize>)>() + 2
}

fn assert_claims(pool: &BudgetPool, rows: usize, index: usize) {
    let stats = pool.stats().unwrap();
    assert_eq!(stats.claimed, rows + index);
    assert_eq!(stats.claimed_for(ResourceKind::InternalRows), rows);
    assert_eq!(stats.claimed_for(ResourceKind::Index), index);
    for kind in [
        ResourceKind::ResponseCache,
        ResourceKind::IndexBuild,
        ResourceKind::ChangeBuffer,
        ResourceKind::Rendering,
    ] {
        assert_eq!(stats.claimed_for(kind), 0);
    }
}

fn assert_rows_and_search(view: &BudgetedPostMetaView, expected: &[PostMetaRow], plan: SearchPlan) {
    assert_eq!(view.rows(), expected);
    for key in [
        None,
        Some(b"".as_slice()),
        Some(b"\0\xff".as_slice()),
        Some(b"absent".as_slice()),
    ] {
        let matches = view.matching_key(key);
        assert_eq!(matches.plan(), plan);
        assert_eq!(
            matches.collect::<Vec<_>>(),
            expected
                .iter()
                .filter(|row| row.meta_key.as_deref() == key)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn borrowed_rows_obey_exact_limits_and_reserve_before_returning_complete_sorted_data() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let pool = BudgetPool::new(charge);
    let mut input = expected.clone();
    input.reverse();
    let view = BudgetedPostMetaView::clone_from_rows(&input, pool.clone(), 3, charge).unwrap();
    assert_rows_and_search(&view, &expected, SearchPlan::Scan);
    assert_claims(&pool, charge, 0);
    drop(view);
    assert_claims(&pool, 0, 0);
    for cap in [0, charge - 1] {
        assert!(
            matches!(BudgetedPostMetaView::clone_from_rows(&input, pool.clone(), 3, cap), Err(BudgetedViewError::Change(LimitedChangeError::ByteLimitExceeded { limit })) if limit == cap)
        );
        assert_claims(&pool, 0, 0);
    }
    assert!(matches!(
        BudgetedPostMetaView::clone_from_rows(&input, pool.clone(), 2, usize::MAX),
        Err(BudgetedViewError::Change(
            LimitedChangeError::RowLimitExceeded { limit: 2 }
        ))
    ));
    let empty = BudgetedPostMetaView::clone_from_rows(&[], pool.clone(), 0, 0).unwrap();
    assert!(empty.rows().is_empty());
    assert_claims(&pool, 0, 0);
    input.reverse();
    assert_eq!(input, expected);
}

#[test]
fn distinct_old_and_new_row_bodies_require_their_combined_charge() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let pool = BudgetPool::new(2 * charge - 1);
    let old =
        BudgetedPostMetaView::clone_from_rows(&expected, pool.clone(), usize::MAX, usize::MAX)
            .unwrap();
    assert!(matches!(
        old.prepare_changes(&[], 3, usize::MAX),
        Err(BudgetedViewError::Budget(BudgetError::Exhausted))
    ));
    assert_rows_and_search(&old, &expected, SearchPlan::Scan);
    assert_claims(&pool, charge, 0);
    pool.set_limit(2 * charge).unwrap();
    let new = old.prepare_changes(&[], 3, charge).unwrap();
    assert_claims(&pool, 2 * charge, 0);
    assert_rows_and_search(&new, &expected, SearchPlan::Scan);
    drop(old);
    assert_claims(&pool, charge, 0);
    drop(new);
    assert_claims(&pool, 0, 0);
}

#[test]
fn arc_sharing_keeps_one_charge_until_the_last_generation_holder_drops() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let index = index_bytes();
    let pool = BudgetPool::new(charge + index);
    let mut view =
        BudgetedPostMetaView::clone_from_rows(&expected, pool.clone(), 3, charge).unwrap();
    assert_eq!(
        view.try_build_key_index(3, index),
        Ok(SearchPlan::ExactKeyIndex)
    );
    let first = Arc::new(view);
    let second = Arc::clone(&first);
    let third = Arc::clone(&second);
    assert_claims(&pool, charge, index);
    drop(first);
    drop(second);
    assert_rows_and_search(&third, &expected, SearchPlan::ExactKeyIndex);
    assert_claims(&pool, charge, index);
    drop(third);
    assert_claims(&pool, 0, 0);
}

#[test]
fn index_rebuild_exhaustion_keeps_old_index_and_needs_both_index_charges() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let index = index_bytes();
    let pool = BudgetPool::new(charge + index);
    let mut view =
        BudgetedPostMetaView::clone_from_rows(&expected, pool.clone(), 3, charge).unwrap();
    assert_eq!(
        view.try_build_key_index(3, index),
        Ok(SearchPlan::ExactKeyIndex)
    );
    assert_eq!(
        view.try_build_key_index(3, index),
        Err(BudgetError::Exhausted)
    );
    assert_claims(&pool, charge, index);
    assert_rows_and_search(&view, &expected, SearchPlan::ExactKeyIndex);
    pool.set_limit(charge + 2 * index).unwrap();
    assert_eq!(
        view.try_build_key_index(usize::MAX, usize::MAX),
        Ok(SearchPlan::ExactKeyIndex)
    );
    assert_claims(&pool, charge, index);
    assert_rows_and_search(&view, &expected, SearchPlan::ExactKeyIndex);
    drop(view);
    assert_claims(&pool, 0, 0);
}

#[test]
fn explicit_scan_fallback_and_discard_release_only_the_index_then_allow_rebuild() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let index = index_bytes();
    let pool = BudgetPool::new(charge + index);
    let mut view =
        BudgetedPostMetaView::clone_from_rows(&expected, pool.clone(), 3, charge).unwrap();
    for (entries, bytes) in [(3, 0), (3, index - 1), (2, usize::MAX)] {
        assert_eq!(
            view.try_build_key_index(3, index),
            Ok(SearchPlan::ExactKeyIndex)
        );
        assert_eq!(
            view.try_build_key_index(entries, bytes),
            Ok(SearchPlan::Scan)
        );
        assert_claims(&pool, charge, 0);
        assert_rows_and_search(&view, &expected, SearchPlan::Scan);
    }
    view.try_build_key_index(3, index).unwrap();
    view.discard_index();
    view.discard_index();
    assert_claims(&pool, charge, 0);
    assert_rows_and_search(&view, &expected, SearchPlan::Scan);
}

#[test]
fn shrinking_limit_keeps_existing_debt_and_rejects_new_bodies_but_allows_zero_candidates() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let index = index_bytes();
    let pool = BudgetPool::new(charge + index);
    let mut old =
        BudgetedPostMetaView::clone_from_rows(&expected, pool.clone(), 3, charge).unwrap();
    old.try_build_key_index(3, index).unwrap();
    pool.set_limit(0).unwrap();
    assert_eq!(pool.stats().unwrap().debt, charge + index);
    assert!(matches!(
        old.prepare_changes(&[MetaChange::Delete(expected[2].clone())], 2, usize::MAX),
        Err(BudgetedViewError::Budget(BudgetError::Exhausted))
    ));
    assert_eq!(
        old.try_build_key_index(3, index),
        Err(BudgetError::Exhausted)
    );
    assert_rows_and_search(&old, &expected, SearchPlan::ExactKeyIndex);
    assert_claims(&pool, charge, index);
    let changes: Vec<_> = expected.iter().cloned().map(MetaChange::Delete).collect();
    let mut empty = old.prepare_changes(&changes, 0, 0).unwrap();
    assert_eq!(
        empty.try_build_key_index(0, 0),
        Ok(SearchPlan::ExactKeyIndex)
    );
    assert_claims(&pool, charge, index);
    old.discard_index();
    assert_eq!(pool.stats().unwrap().debt, charge);
    drop(old);
    assert_claims(&pool, 0, 0);
    assert_eq!(pool.stats().unwrap().debt, 0);
    drop(empty);
    assert_claims(&pool, 0, 0);
}

#[test]
fn strict_failures_precede_limits_and_budget_without_mutating_old_rows_index_or_claims() {
    let expected = rows();
    let charge = row_bytes(&expected);
    let index = index_bytes();
    let pool = BudgetPool::new(charge + index);
    let mut old =
        BudgetedPostMetaView::clone_from_rows(&expected, pool.clone(), 3, charge).unwrap();
    old.try_build_key_index(3, index).unwrap();
    let mut wrong = expected[0].clone();
    wrong.meta_key = Some(vec![]);
    let mut missing = expected[0].clone();
    missing.meta_id = 17;
    for (change, error) in [
        (
            MetaChange::Insert(expected[0].clone()),
            ChangeError::DuplicateId(0),
        ),
        (
            MetaChange::Delete(wrong),
            ChangeError::BeforeImageMismatch(0),
        ),
        (MetaChange::Delete(missing), ChangeError::MissingRow(17)),
        (
            MetaChange::Update {
                before: expected[0].clone(),
                after: expected[1].clone(),
            },
            ChangeError::DuplicateId(1),
        ),
    ] {
        assert!(
            matches!(old.prepare_changes(&[change], 0, 0), Err(BudgetedViewError::Change(LimitedChangeError::Change(actual))) if actual == error)
        );
        assert_claims(&pool, charge, index);
        assert_rows_and_search(&old, &expected, SearchPlan::ExactKeyIndex);
    }
    let duplicate = [expected[0].clone(), expected[0].clone()];
    assert!(matches!(
        BudgetedPostMetaView::clone_from_rows(&duplicate, pool.clone(), 0, 0),
        Err(BudgetedViewError::Change(LimitedChangeError::Change(
            ChangeError::DuplicateId(0)
        )))
    ));
    assert_claims(&pool, charge, index);
    let error = BudgetedViewError::Budget(BudgetError::Unavailable);
    fn is_error(_: &dyn std::error::Error) {}
    is_error(&error);
    assert!(!error.to_string().contains("語"));
}

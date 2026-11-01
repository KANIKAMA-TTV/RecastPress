use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind as Kind};
use std::sync::{Arc, Barrier};

const KINDS: [Kind; 6] = [
    Kind::ResponseCache,
    Kind::InternalRows,
    Kind::Index,
    Kind::IndexBuild,
    Kind::ChangeBuffer,
    Kind::Rendering,
];

fn conserved(pool: &BudgetPool) {
    let stats = pool.stats().unwrap();
    assert_eq!(
        KINDS
            .iter()
            .map(|&kind| stats.claimed_for(kind))
            .sum::<usize>(),
        stats.claimed
    );
    assert_eq!(stats.available, stats.limit.saturating_sub(stats.claimed));
    assert_eq!(stats.debt, stats.claimed.saturating_sub(stats.limit));
}

#[test]
fn categories_and_cloned_handles_share_one_move_only_reservation_pool() {
    let pool = BudgetPool::new(60);
    let clone = pool.clone();
    let mut leases = Vec::new();
    for kind in KINDS {
        leases.push(clone.try_reserve(kind, 10).unwrap());
    }
    assert_eq!(pool.stats().unwrap().claimed, 60);
    assert!(matches!(
        pool.try_reserve(Kind::Index, 1),
        Err(BudgetError::Exhausted)
    ));
    conserved(&pool);
    let moved = leases.pop().unwrap();
    assert_eq!(moved.kind(), Kind::Rendering);
    assert_eq!(pool.stats().unwrap().claimed, 60);
    drop(moved);
    assert_eq!(pool.stats().unwrap().claimed_for(Kind::Rendering), 0);
    drop(leases);
    assert_eq!(clone.stats().unwrap().claimed, 0);
    conserved(&pool);
}

#[test]
fn failed_growth_is_atomic_and_does_not_change_other_consumers() {
    let pool = BudgetPool::new(100);
    let mut cache = pool.try_reserve(Kind::ResponseCache, 60).unwrap();
    let index = pool.try_reserve(Kind::Index, 30).unwrap();
    let before = pool.stats().unwrap();
    assert_eq!(cache.try_resize(71), Err(BudgetError::Exhausted));
    assert_eq!(cache.bytes(), 60);
    assert_eq!(pool.stats().unwrap(), before);
    assert_eq!(index.bytes(), 30);
    cache.try_resize(70).unwrap();
    assert_eq!(pool.stats().unwrap().available, 0);
    conserved(&pool);
}

#[test]
fn limit_reduction_keeps_debt_until_holders_actually_release_it() {
    let pool = BudgetPool::new(10);
    let mut rows = pool.try_reserve(Kind::InternalRows, 10).unwrap();
    pool.set_limit(5).unwrap();
    assert_eq!(pool.stats().unwrap().debt, 5);
    assert!(matches!(
        pool.try_reserve(Kind::IndexBuild, 1),
        Err(BudgetError::Exhausted)
    ));
    assert_eq!(rows.try_resize(11), Err(BudgetError::Exhausted));
    rows.try_resize(8).unwrap();
    assert_eq!(pool.stats().unwrap().claimed, 8);
    assert_eq!(pool.stats().unwrap().debt, 3);
    rows.try_resize(8).unwrap();
    rows.try_resize(5).unwrap();
    assert_eq!(pool.stats().unwrap().debt, 0);
    pool.set_limit(0).unwrap();
    assert_eq!(pool.stats().unwrap().debt, 5);
    rows.try_resize(0).unwrap();
    assert_eq!(pool.stats().unwrap().claimed, 0);
    conserved(&pool);
}

#[test]
fn usize_max_reservations_do_not_wrap_and_zero_does_not_add_a_charge() {
    let pool = BudgetPool::new(usize::MAX);
    let mut buffer = pool.try_reserve(Kind::ChangeBuffer, usize::MAX).unwrap();
    assert!(matches!(
        pool.try_reserve(Kind::Index, 1),
        Err(BudgetError::Exhausted)
    ));
    let empty = pool.try_reserve(Kind::Rendering, 0).unwrap();
    assert_eq!(empty.bytes(), 0);
    buffer.try_resize(usize::MAX - 1).unwrap();
    let last = pool.try_reserve(Kind::IndexBuild, 1).unwrap();
    let before = pool.stats().unwrap();
    assert_eq!(buffer.try_resize(usize::MAX), Err(BudgetError::Exhausted));
    assert_eq!(pool.stats().unwrap(), before);
    drop(last);
    buffer.try_resize(usize::MAX).unwrap();
    pool.set_limit(0).unwrap();
    let zero_in_debt = pool.try_reserve(Kind::Index, 0).unwrap();
    assert_eq!(pool.stats().unwrap().debt, usize::MAX);
    drop((buffer, empty, zero_in_debt));
    conserved(&pool);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn concurrent_clones_cannot_each_spend_the_same_available_bytes() {
    let pool = BudgetPool::new(128);
    let start = Arc::new(Barrier::new(17));
    let workers: Vec<_> = (0..16)
        .map(|_| {
            let pool = pool.clone();
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                pool.try_reserve(Kind::IndexBuild, 16)
            })
        })
        .collect();
    start.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 8);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(BudgetError::Exhausted)))
            .count(),
        8
    );
    assert_eq!(pool.stats().unwrap().claimed, 128);
    conserved(&pool);
    drop(results);
    assert_eq!(pool.stats().unwrap().available, 128);
    conserved(&pool);
}

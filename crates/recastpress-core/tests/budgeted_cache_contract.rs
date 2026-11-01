use recastpress_core::{
    cache::{BudgetedPageCache, CacheKey, MonitorState, PageCache, StopReason},
    resource_budget::{BudgetError, BudgetPool, ResourceKind},
};

fn key(target: &str) -> CacheKey {
    CacheKey {
        site: "site".into(),
        host: "example.test".into(),
        target: target.into(),
        representation: "html".into(),
    }
}

fn ready(pool: BudgetPool, local: usize) -> BudgetedPageCache {
    let mut cache = BudgetedPageCache::new(pool);
    cache.apply_budget(local);
    cache.reconciliation_completed();
    cache
}

fn charge_for(target: &str, body: &[u8]) -> usize {
    let pool = BudgetPool::new(usize::MAX);
    let mut cache = ready(pool.clone(), usize::MAX);
    let ticket = cache.ticket().unwrap();
    assert_eq!(cache.insert(&ticket, key(target), body.to_vec()), Ok(true));
    let charge = cache.status().charged_bytes;
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::ResponseCache),
        charge
    );
    drop(cache);
    assert_eq!(pool.stats().unwrap().claimed, 0);
    charge
}

#[test]
fn existing_stop_epoch_ticket_and_local_refusal_rules_precede_global_admission() {
    let pool = BudgetPool::new(0);
    let mut cache = BudgetedPageCache::new(pool.clone());
    assert_eq!(cache.status().monitor, MonitorState::Reconciling);
    assert_eq!(
        cache.status().stop_reason,
        Some(StopReason::MonitorNotReady)
    );
    assert!(cache.ticket().is_none());
    cache.reconciliation_completed();
    assert_eq!(cache.status().stop_reason, Some(StopReason::NoBudget));
    cache.apply_budget(1024);
    let ticket = cache.ticket().unwrap();
    assert_eq!(cache.insert(&ticket, key("/"), vec![0; 2048]), Ok(false));
    assert_eq!(
        cache.insert(&ticket, key("/"), vec![0]),
        Err(BudgetError::Exhausted)
    );
    cache.set_enabled(false);
    assert_eq!(cache.status().stop_reason, Some(StopReason::Disabled));
    assert_eq!(cache.insert(&ticket, key("/"), vec![0]), Ok(false));
    cache.set_enabled(true);
    assert!(!cache.is_current(&ticket));
    let unrelated = ready(pool.clone(), 1024).ticket().unwrap();
    assert_eq!(cache.insert(&unrelated, key("/"), vec![0]), Ok(false));
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn enlarged_entry_charge_is_shared_with_legacy_model_and_spare_capacities_count() {
    fn input() -> (CacheKey, Vec<u8>) {
        let mut key = key("/spare");
        key.target.reserve(511);
        let mut body = Vec::with_capacity(511);
        body.push(7);
        (key, body)
    }
    let baseline = charge_for("/spare", &[7]);
    let (stored_key, body) = input();
    let extra =
        stored_key.target.capacity() - stored_key.target.len() + body.capacity() - body.len();
    let pool = BudgetPool::new(usize::MAX);
    let mut cache = ready(pool.clone(), usize::MAX);
    let ticket = cache.ticket().unwrap();
    assert_eq!(cache.insert(&ticket, stored_key, body), Ok(true));
    let charge = cache.status().charged_bytes;
    assert_eq!(charge, baseline + extra);
    assert_eq!(pool.stats().unwrap().claimed, charge);
    let mut legacy = PageCache::new();
    legacy.apply_budget(usize::MAX);
    legacy.reconciliation_completed();
    let (same_key, same_body) = input();
    assert!(legacy.insert(&legacy.ticket().unwrap(), same_key, same_body));
    assert_eq!(legacy.status().charged_bytes, charge);
    assert_eq!(cache.get(&key("/spare")), Some([7].as_slice()));
    drop(cache);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn cloned_handles_hold_full_entry_charge_after_purge_and_cache_drop() {
    let charge = charge_for("/", b"public");
    let pool = BudgetPool::new(charge);
    let mut cache = ready(pool.clone(), charge);
    let ticket = cache.ticket().unwrap();
    cache.insert(&ticket, key("/"), b"public".to_vec()).unwrap();
    let first = cache.get_shared(&key("/")).unwrap();
    let second = first.clone();
    let third = cache.get_shared(&key("/")).unwrap();
    assert_eq!(first.as_bytes().as_ptr(), third.as_bytes().as_ptr());
    assert_eq!(first.charged_bytes(), charge);
    assert!(charge > first.as_bytes().len());
    cache.purge();
    assert_eq!(cache.status().charged_bytes, 0);
    assert_eq!(cache.status().entries, 0);
    assert!(!cache.is_current(&ticket));
    assert!(cache.get_shared(&key("/")).is_none());
    assert_eq!(pool.stats().unwrap().claimed, charge);
    drop(cache);
    drop(first);
    drop(third);
    assert_eq!(second.as_bytes(), b"public");
    assert_eq!(pool.stats().unwrap().claimed, charge);
    drop(second);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn rejected_replacement_keeps_old_generation_and_success_needs_old_plus_new_charge() {
    let old_charge = charge_for("/same", b"old");
    let new_charge = charge_for("/same", b"new body");
    let pool = BudgetPool::new(old_charge + new_charge - 1);
    let mut cache = ready(pool.clone(), usize::MAX);
    let ticket = cache.ticket().unwrap();
    cache
        .insert(&ticket, key("/same"), b"old".to_vec())
        .unwrap();
    let old = cache.get_shared(&key("/same")).unwrap();
    let before = cache.status();
    assert_eq!(
        cache.insert(&ticket, key("/same"), b"new body".to_vec()),
        Err(BudgetError::Exhausted)
    );
    assert_eq!(cache.status(), before);
    assert!(cache.is_current(&ticket));
    assert_eq!(cache.get(&key("/same")), Some(b"old".as_slice()));
    pool.set_limit(old_charge + new_charge).unwrap();
    assert_eq!(
        cache.insert(&ticket, key("/same"), b"new body".to_vec()),
        Ok(true)
    );
    assert_eq!(cache.status().charged_bytes, new_charge);
    assert_eq!(pool.stats().unwrap().claimed, old_charge + new_charge);
    assert_eq!(old.as_bytes(), b"old");
    assert_eq!(cache.get(&key("/same")), Some(b"new body".as_slice()));
    drop(old);
    assert_eq!(pool.stats().unwrap().claimed, new_charge);
    drop(cache);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn global_refusal_cannot_touch_lru_or_evict_to_fund_the_failed_admission() {
    let charge = charge_for("/a", b"x");
    let pool = BudgetPool::new(2 * charge);
    let mut cache = ready(pool.clone(), 2 * charge);
    let ticket = cache.ticket().unwrap();
    cache.insert(&ticket, key("/a"), b"x".to_vec()).unwrap();
    cache.insert(&ticket, key("/b"), b"x".to_vec()).unwrap();
    assert!(cache.get(&key("/a")).is_some()); // b is now oldest.
    assert_eq!(
        cache.insert(&ticket, key("/b"), b"y".to_vec()),
        Err(BudgetError::Exhausted)
    );
    assert_eq!(cache.status().entries, 2);
    assert_eq!(pool.stats().unwrap().claimed, 2 * charge);
    pool.set_limit(3 * charge).unwrap();
    cache.insert(&ticket, key("/c"), b"x".to_vec()).unwrap();
    assert!(cache.get(&key("/b")).is_none());
    assert!(cache.get(&key("/a")).is_some());
    assert!(cache.get(&key("/c")).is_some());
    assert_eq!(pool.stats().unwrap().claimed, 2 * charge);
}

#[test]
fn evicted_handle_retains_charge_while_local_status_counts_only_resident_entries() {
    let charge = charge_for("/a", b"x");
    let pool = BudgetPool::new(3 * charge);
    let mut cache = ready(pool.clone(), charge);
    let ticket = cache.ticket().unwrap();
    cache.insert(&ticket, key("/a"), b"x".to_vec()).unwrap();
    let retained = cache.get_shared(&key("/a")).unwrap();
    cache.insert(&ticket, key("/b"), b"x".to_vec()).unwrap();
    assert!(cache.get(&key("/a")).is_none());
    assert_eq!(cache.status().charged_bytes, charge);
    assert_eq!(pool.stats().unwrap().claimed, 2 * charge);
    assert_eq!(retained.as_bytes(), b"x");
    drop(retained);
    assert_eq!(pool.stats().unwrap().claimed, charge);
    drop(cache);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn monitor_controls_and_local_shrink_invalidate_tickets_but_not_retained_lease_ownership() {
    let charge = charge_for("/", b"x");
    for control in 0..6 {
        let pool = BudgetPool::new(charge);
        let mut cache = ready(pool.clone(), charge);
        let ticket = cache.ticket().unwrap();
        cache.insert(&ticket, key("/"), b"x".to_vec()).unwrap();
        let retained = cache.get_shared(&key("/")).unwrap();
        match control {
            0 => cache.purge(),
            1 => cache.monitoring_lost(),
            2 => cache.begin_reconciliation(),
            3 => cache.reconciliation_completed(),
            4 => cache.set_enabled(false),
            _ => cache.apply_budget(0),
        }
        assert!(!cache.is_current(&ticket));
        assert_eq!(cache.status().entries, 0);
        assert_eq!(cache.status().charged_bytes, 0);
        assert_eq!(retained.as_bytes(), b"x");
        assert_eq!(pool.stats().unwrap().claimed, charge);
        drop(cache);
        assert_eq!(pool.stats().unwrap().claimed, charge);
        drop(retained);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

#[test]
fn pool_shrink_records_debt_without_revoking_reads_or_changing_monitor_state() {
    let charge = charge_for("/", b"x");
    let pool = BudgetPool::new(charge);
    let mut cache = ready(pool.clone(), charge);
    let ticket = cache.ticket().unwrap();
    cache.insert(&ticket, key("/"), b"x".to_vec()).unwrap();
    let retained = cache.get_shared(&key("/")).unwrap();
    pool.set_limit(0).unwrap();
    assert_eq!(pool.stats().unwrap().debt, charge);
    assert!(cache.is_current(&ticket));
    assert_eq!(cache.status().monitor, MonitorState::Tracking);
    assert_eq!(cache.get(&key("/")), Some(b"x".as_slice()));
    assert_eq!(
        cache.insert(&ticket, key("/"), b"y".to_vec()),
        Err(BudgetError::Exhausted)
    );
    cache.purge();
    drop(cache);
    assert_eq!(retained.as_bytes(), b"x");
    assert_eq!(pool.stats().unwrap().debt, charge);
    drop(retained);
    assert_eq!(pool.stats().unwrap().claimed, 0);
    assert_eq!(pool.stats().unwrap().debt, 0);
}

#[test]
fn response_cache_admission_competes_with_other_categories_and_releases_only_its_claims() {
    let charge = charge_for("/", b"x");
    let pool = BudgetPool::new(charge + 6);
    let other = pool.try_reserve(ResourceKind::Rendering, 7).unwrap();
    let mut cache = ready(pool.clone(), charge);
    let ticket = cache.ticket().unwrap();
    assert_eq!(
        cache.insert(&ticket, key("/"), b"x".to_vec()),
        Err(BudgetError::Exhausted)
    );
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::ResponseCache),
        0
    );
    pool.set_limit(charge + 7).unwrap();
    cache.insert(&ticket, key("/"), b"x".to_vec()).unwrap();
    assert_eq!(pool.stats().unwrap().claimed, charge + 7);
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::ResponseCache),
        charge
    );
    cache.purge();
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::ResponseCache),
        0
    );
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        7
    );
    drop(cache);
    drop(other);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

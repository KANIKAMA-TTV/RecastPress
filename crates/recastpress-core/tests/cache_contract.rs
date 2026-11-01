use recastpress_core::cache::{CacheKey, MonitorState, PageCache, StopReason};
use recastpress_core::settings::{SiteSettings, SsoPolicy};

fn key(target: &str) -> CacheKey {
    CacheKey {
        site: "site-1".into(),
        host: "example.test".into(),
        target: target.into(),
        representation: "html".into(),
    }
}

fn ready(budget: usize) -> PageCache {
    let mut cache = PageCache::new();
    cache.apply_budget(budget);
    cache.reconciliation_completed();
    cache
}

#[test]
fn initial_settings_and_imported_policy_are_distinct() {
    assert_eq!(
        SiteSettings::default(),
        SiteSettings {
            cache_enabled: true,
            sso: SsoPolicy::Off,
            comments_open: false
        }
    );
    assert!(SiteSettings::from_migration(true, true, SsoPolicy::Off).comments_open);
}

#[test]
fn startup_needs_both_monitor_and_finite_budget() {
    let mut cache = PageCache::new();
    assert!(cache.status().configured_enabled);
    assert_eq!(
        cache.status().stop_reason,
        Some(StopReason::MonitorNotReady)
    );
    cache.reconciliation_completed();
    assert_eq!(cache.status().stop_reason, Some(StopReason::NoBudget));
    cache.apply_budget(1024);
    assert!(cache.ticket().is_some());
}

#[test]
fn query_bytes_are_not_normalized_and_all_variants_are_purged() {
    let mut cache = ready(10_000);
    let ticket = cache.ticket().unwrap();
    let targets = [
        "/?a=1&a=2",
        "/?a=2&a=1",
        "/?a=%20",
        "/?a=+",
        "/?a=",
        "/?a",
        "/",
    ];
    for (i, target) in targets.iter().enumerate() {
        assert!(cache.insert(&ticket, key(target), vec![i as u8]));
    }
    for (i, target) in targets.iter().enumerate() {
        assert_eq!(cache.get(&key(target)), Some([i as u8].as_slice()));
    }
    cache.purge();
    assert_eq!(cache.status().entries, 0);
    assert_eq!(cache.status().charged_bytes, 0);
    assert!(!cache.is_current(&ticket));
    assert!(!cache.insert(&ticket, key("/late"), vec![1]));
}

#[test]
fn monitor_loss_stops_hits_and_writes_without_changing_preference() {
    let mut cache = ready(1024);
    let ticket = cache.ticket().unwrap();
    assert!(cache.insert(&ticket, key("/"), b"public".to_vec()));
    cache.monitoring_lost();
    assert!(cache.status().configured_enabled);
    assert_eq!(cache.status().monitor, MonitorState::Unavailable);
    assert!(cache.get(&key("/")).is_none());
    assert!(!cache.insert(&ticket, key("/"), b"late".to_vec()));
    cache.begin_reconciliation();
    assert!(cache.ticket().is_none());
    cache.reconciliation_completed();
    assert!(cache.ticket().is_some());
    assert!(!cache.is_current(&ticket));
    assert_eq!(cache.status().entries, 0);
}

#[test]
fn tickets_cannot_cross_cache_instances_or_off_on_cycles() {
    let mut first = ready(1024);
    let second = ready(1024);
    let old = first.ticket().unwrap();
    assert!(!second.is_current(&old));
    first.set_enabled(false);
    first.set_enabled(true);
    assert!(!first.is_current(&old));
}

#[test]
fn shrinking_budget_rejects_pending_work_and_zero_stops_cache() {
    let mut cache = ready(1024);
    let ticket = cache.ticket().unwrap();
    assert!(cache.insert(&ticket, key("/"), vec![0; 20]));
    cache.apply_budget(512);
    assert!(!cache.is_current(&ticket));
    assert_eq!(cache.status().charged_bytes, 0);
    cache.apply_budget(0);
    assert!(cache.ticket().is_none());
}

#[test]
fn oversize_replacement_keeps_old_value_and_eviction_obeys_budget() {
    let mut cache = ready(1024);
    let ticket = cache.ticket().unwrap();
    assert!(cache.insert(&ticket, key("/"), vec![1; 20]));
    assert!(!cache.insert(&ticket, key("/"), vec![2; 2048]));
    assert_eq!(cache.get(&key("/")), Some([1; 20].as_slice()));
    for i in 0..1000 {
        assert!(cache.insert(&ticket, key(&format!("/?id={i}")), vec![3; 100]));
        assert!(cache.status().charged_bytes <= cache.status().budget_bytes);
    }
    assert!(cache.get(&key("/?id=999")).is_some());
    assert!(cache.get(&key("/")).is_none());
}

#[test]
fn spare_allocations_cannot_bypass_logical_budget() {
    let mut cache = ready(1024);
    let ticket = cache.ticket().unwrap();
    let mut body = Vec::with_capacity(2048);
    body.push(1);
    assert!(!cache.insert(&ticket, key("/"), body));
    let mut large_key = key("/");
    large_key.target.reserve(2048);
    assert!(!cache.insert(&ticket, large_key, vec![1]));
    assert_eq!(cache.status().charged_bytes, 0);
}

#[test]
fn shared_body_handles_survive_purge_but_cannot_authorize_delivery() {
    let mut cache = ready(1024);
    let ticket = cache.ticket().unwrap();
    assert!(cache.insert(&ticket, key("/"), b"public".to_vec()));
    let first = cache.get_shared(&key("/")).unwrap();
    let second = cache.get_shared(&key("/")).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    cache.purge();
    assert!(cache.get_shared(&key("/")).is_none());
    assert!(!cache.is_current(&ticket));
    // Already retained bytes still exist; their owner must check its own token.
    assert_eq!(&*first, b"public");
}

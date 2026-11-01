//! In-process cache state machine (R42, R49, R50).
//!
//! No TTL exists. A monitor must complete reconciliation before caching starts.
//! This is not a DB/file watcher or a distributed invalidation protocol.

use crate::resource_budget::{BudgetError, BudgetPool, Reservation, ResourceKind};
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonitorState {
    Reconciling,
    Tracking,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Disabled,
    MonitorNotReady,
    NoBudget,
}

/// Exact request target bytes are retained: query order, repeats, and encoding differ.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey {
    pub site: String,
    pub host: String,
    pub target: String,
    pub representation: String,
}

impl CacheKey {
    fn bytes(&self) -> Option<usize> {
        self.site
            .capacity()
            .checked_add(self.host.capacity())?
            .checked_add(self.target.capacity())?
            .checked_add(self.representation.capacity())
    }
}

/// Unforgeable outside this module. Invalidated by purge, stop, or recovery.
#[derive(Clone, Debug)]
pub struct CacheTicket {
    epoch: Arc<()>,
}

impl PartialEq for CacheTicket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.epoch, &other.epoch)
    }
}

impl Eq for CacheTicket {}

#[derive(Debug)]
struct Entry {
    body: Arc<[u8]>,
    charge: usize,
    used: u64,
    // Last: destroy this body's reference before returning shared credit.
    // The actual enlarged header participates in BOTH cache charge models.
    lease: Option<Arc<Reservation>>,
}

/// Immutable bytes and the SAME full-entry lease as their cache generation.
/// Clone shares both references. Body drops before lease; no bare Arc/Vec or
/// mutable/owned escape is provided. Explicit caller copies are unaccounted.
/// This handle is not a freshness token and cannot authorize public delivery.
#[derive(Clone)]
pub struct BudgetedCachedBody {
    body: Arc<[u8]>,
    lease: Arc<Reservation>,
}

impl BudgetedCachedBody {
    pub fn as_bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn charged_bytes(&self) -> usize {
        self.lease.bytes()
    }
}

impl fmt::Debug for BudgetedCachedBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BudgetedCachedBody")
            .field("body_bytes", &self.body.len())
            .field("entry_charge", &self.charged_bytes())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheStatus {
    pub configured_enabled: bool,
    pub monitor: MonitorState,
    pub stop_reason: Option<StopReason>,
    pub budget_bytes: usize,
    pub charged_bytes: usize,
    pub entries: usize,
}

/// Budget is supplied by an internal resource allocator, never a user setting.
/// Charges include keys, bodies, and a conservative per-entry accounting allowance;
/// this is a logical budget, not an allocator/RSS measurement.
#[derive(Debug)]
pub struct PageCache {
    enabled: bool,
    monitor: MonitorState,
    budget: usize,
    charged: usize,
    epoch: Arc<()>,
    clock: u64,
    entries: HashMap<CacheKey, Entry>,
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PageCache {
    pub fn new() -> Self {
        Self {
            enabled: true,
            monitor: MonitorState::Reconciling,
            budget: 0,
            charged: 0,
            epoch: Arc::new(()),
            clock: 0,
            entries: HashMap::new(),
        }
    }

    pub fn status(&self) -> CacheStatus {
        let stop_reason = if !self.enabled {
            Some(StopReason::Disabled)
        } else if self.monitor != MonitorState::Tracking {
            Some(StopReason::MonitorNotReady)
        } else if self.budget == 0 {
            Some(StopReason::NoBudget)
        } else {
            None
        };
        CacheStatus {
            configured_enabled: self.enabled,
            monitor: self.monitor,
            stop_reason,
            budget_bytes: self.budget,
            charged_bytes: self.charged,
            entries: self.entries.len(),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.purge();
            self.enabled = enabled;
        }
    }

    /// Called on loss of any required DB/file/generation monitor.
    pub fn monitoring_lost(&mut self) {
        self.purge();
        self.monitor = MonitorState::Unavailable;
    }

    pub fn begin_reconciliation(&mut self) {
        self.purge();
        self.monitor = MonitorState::Reconciling;
    }

    /// Caller must have verified all required sources and the public generation.
    pub fn reconciliation_completed(&mut self) {
        self.purge();
        self.monitor = MonitorState::Tracking;
    }

    /// Invalidates outstanding work before releasing all cached entries.
    pub fn purge(&mut self) {
        self.epoch = Arc::new(());
        self.clear_entries();
    }

    fn clear_entries(&mut self) {
        // Full-entry leases include the stored key. Do not depend on HashMap's
        // internal K/V drop order; bucket allocation itself is unpooled.
        for (key, entry) in self.entries.drain() {
            drop(key);
            drop(entry);
        }
        self.entries.shrink_to_fit();
        self.charged = 0;
    }

    /// Applies an automatically computed local allocation. Zero suspends caching.
    pub fn apply_budget(&mut self, bytes: usize) {
        if bytes < self.budget {
            // Pending renders might have been admitted under the old allocation.
            self.purge();
        }
        self.budget = bytes;
    }

    pub fn ticket(&self) -> Option<CacheTicket> {
        self.status().stop_reason.is_none().then(|| CacheTicket {
            epoch: Arc::clone(&self.epoch),
        })
    }

    pub fn is_current(&self, ticket: &CacheTicket) -> bool {
        self.enabled
            && self.monitor == MonitorState::Tracking
            && self.budget > 0
            && Arc::ptr_eq(&self.epoch, &ticket.epoch)
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<&[u8]> {
        self.touch_entry(key).map(|entry| entry.body.as_ref())
    }

    /// Shares immutable bytes without copying the body while holding a cache lock.
    /// A retained handle is not a freshness token; delivery must still obey the
    /// caller's publication and invalidation contract.
    pub fn get_shared(&mut self, key: &CacheKey) -> Option<Arc<[u8]>> {
        self.touch_entry(key).map(|entry| Arc::clone(&entry.body))
    }

    fn touch_entry(&mut self, key: &CacheKey) -> Option<&Entry> {
        self.ticket()?;
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.used = self.clock;
        Some(entry)
    }

    /// Returns false for stopped/stale/oversize work. Existing values survive a
    /// rejected replacement. Callers must separately check the ticket at delivery.
    pub fn insert(&mut self, ticket: &CacheTicket, key: CacheKey, body: Vec<u8>) -> bool {
        match self.insert_with_admission(ticket, key, body, |_| {
            Ok::<_, std::convert::Infallible>(None)
        }) {
            Ok(inserted) => inserted,
            Err(never) => match never {},
        }
    }

    /// Shared local gate/charge/eviction logic. Admission occurs before ANY
    /// replacement, LRU clock update or eviction. Inputs already belong to the
    /// caller; admission accounts retention, not their original allocation.
    fn insert_with_admission<E>(
        &mut self,
        ticket: &CacheTicket,
        key: CacheKey,
        body: Vec<u8>,
        admit: impl FnOnce(usize) -> Result<Option<Arc<Reservation>>, E>,
    ) -> Result<bool, E> {
        if !self.is_current(ticket) {
            return Ok(false);
        }
        let Some(charge) = key
            .bytes()
            .and_then(|n| n.checked_add(body.capacity()))
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<Entry>() + std::mem::size_of::<CacheKey>() + 64)
            })
        else {
            return Ok(false);
        };
        if charge > self.budget {
            return Ok(false);
        }
        let lease = admit(charge)?;
        if let Some((old_key, old)) = self.entries.remove_entry(&key) {
            self.charged -= old.charge;
            drop(old_key);
            drop(old);
        }
        while self.charged > self.budget - charge {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some((old_key, old)) = self.entries.remove_entry(&oldest) {
                self.charged -= old.charge;
                drop(old_key);
                drop(old);
            }
        }
        self.clock = self.clock.saturating_add(1);
        self.entries.insert(
            key,
            Entry {
                body: body.into(),
                charge,
                used: self.clock,
                lease,
            },
        );
        self.charged += charge;
        Ok(true)
    }
}

impl Drop for PageCache {
    fn drop(&mut self) {
        self.clear_entries();
    }
}

/// Explicit one-process shared-pool facade. It does not connect the existing
/// HTTP/CLI cache, select allowances, add TTLs or change monitor/epoch policy.
/// Local status counts resident entries; pool charges also include full-entry
/// leases retained by external handles after eviction/purge/drop. Retaining key
/// and spare-input capacity charges after their release is conservative.
///
/// The Entry header now includes an optional lease pointer, so both legacy and
/// budgeted logical charges reflect that enlarged header. This can change local
/// byte thresholds; gate/stop/LRU/oversize-replacement rules remain the same.
/// Incoming keys/payload allocation, Arc wrappers, explicit/eviction-key copies,
/// HashMap bucket capacity/allocator overhead, peak memory and RSS are not
/// covered. Poisoned pools keep
/// conservative charges and are not recovered by this owner.
pub struct BudgetedPageCache {
    inner: PageCache,
    pool: BudgetPool,
}

impl fmt::Debug for BudgetedPageCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BudgetedPageCache")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl BudgetedPageCache {
    pub fn new(pool: BudgetPool) -> Self {
        Self {
            inner: PageCache::new(),
            pool,
        }
    }

    pub fn status(&self) -> CacheStatus {
        self.inner.status()
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }
    pub fn monitoring_lost(&mut self) {
        self.inner.monitoring_lost();
    }
    pub fn begin_reconciliation(&mut self) {
        self.inner.begin_reconciliation();
    }
    pub fn reconciliation_completed(&mut self) {
        self.inner.reconciliation_completed();
    }
    pub fn purge(&mut self) {
        self.inner.purge();
    }
    pub fn apply_budget(&mut self, bytes: usize) {
        self.inner.apply_budget(bytes);
    }
    pub fn ticket(&self) -> Option<CacheTicket> {
        self.inner.ticket()
    }
    pub fn is_current(&self, ticket: &CacheTicket) -> bool {
        self.inner.is_current(ticket)
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<&[u8]> {
        self.inner.get(key)
    }

    pub fn get_shared(&mut self, key: &CacheKey) -> Option<BudgetedCachedBody> {
        let entry = self.inner.touch_entry(key)?;
        // Only admitted entries can be present through this private facade.
        // Fail closed rather than exposing an unleased body if that changes.
        let lease = entry.lease.as_ref()?;
        Some(BudgetedCachedBody {
            body: Arc::clone(&entry.body),
            lease: Arc::clone(lease),
        })
    }

    /// Local stopped/stale/oversize refusal is Ok(false). A pool error preserves
    /// every old value and LRU position; no eviction is used to fund admission.
    /// Old/new generations therefore need their combined live charges first.
    pub fn insert(
        &mut self,
        ticket: &CacheTicket,
        key: CacheKey,
        body: Vec<u8>,
    ) -> Result<bool, BudgetError> {
        let pool = &self.pool;
        self.inner
            .insert_with_admission(ticket, key, body, |charge| {
                Ok(Some(Arc::new(
                    pool.try_reserve(ResourceKind::ResponseCache, charge)?,
                )))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_admission_keeps_old_value_ticket_lru_clock_and_charge() {
        let mut cache = PageCache::new();
        cache.apply_budget(4096);
        cache.reconciliation_completed();
        let ticket = cache.ticket().unwrap();
        let key = CacheKey {
            site: "s".into(),
            host: "h".into(),
            target: "/".into(),
            representation: "html".into(),
        };
        assert!(cache.insert(&ticket, key.clone(), b"old".to_vec()));
        let before_clock = cache.clock;
        let before_used = cache.entries[&key].used;
        let before_status = cache.status();
        assert_eq!(
            cache.insert_with_admission(&ticket, key.clone(), b"new".to_vec(), |_| Err::<
                Option<Arc<Reservation>>,
                _,
            >(
                BudgetError::Unavailable
            )),
            Err(BudgetError::Unavailable)
        );
        assert_eq!(cache.clock, before_clock);
        assert_eq!(cache.entries[&key].used, before_used);
        assert_eq!(cache.status(), before_status);
        assert!(cache.is_current(&ticket));
        assert_eq!(cache.get(&key), Some(b"old".as_slice()));
    }
}

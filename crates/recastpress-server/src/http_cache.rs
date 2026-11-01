//! Shared body cache for the sealed startup generation only.
//!
//! This adapter does not implement resource discovery or change monitoring. The
//! default remains suspended until an internal allocator and monitor supply both.
//! It must not be reused for live publication without a transport revocation contract.

use std::sync::{Arc, Mutex};

use axum::{
    body::{Body, Bytes, HttpBody},
    extract::Request,
    http::Method,
};
use recastpress_core::{
    cache::{BudgetedCachedBody, BudgetedPageCache, CacheKey, CacheStatus, CacheTicket, PageCache},
    publication::PublicPage,
    resource_budget::{BudgetError, BudgetPool},
};

enum CacheBackend {
    Legacy(PageCache),
    Budgeted(BudgetedPageCache),
}

impl CacheBackend {
    fn status(&self) -> CacheStatus {
        match self {
            Self::Legacy(cache) => cache.status(),
            Self::Budgeted(cache) => cache.status(),
        }
    }
    fn set_enabled(&mut self, enabled: bool) {
        match self {
            Self::Legacy(cache) => cache.set_enabled(enabled),
            Self::Budgeted(cache) => cache.set_enabled(enabled),
        }
    }
    fn apply_budget(&mut self, bytes: usize) {
        match self {
            Self::Legacy(cache) => cache.apply_budget(bytes),
            Self::Budgeted(cache) => cache.apply_budget(bytes),
        }
    }
    fn monitoring_lost(&mut self) {
        match self {
            Self::Legacy(cache) => cache.monitoring_lost(),
            Self::Budgeted(cache) => cache.monitoring_lost(),
        }
    }
    fn begin_reconciliation(&mut self) {
        match self {
            Self::Legacy(cache) => cache.begin_reconciliation(),
            Self::Budgeted(cache) => cache.begin_reconciliation(),
        }
    }
    fn reconciliation_completed(&mut self) {
        match self {
            Self::Legacy(cache) => cache.reconciliation_completed(),
            Self::Budgeted(cache) => cache.reconciliation_completed(),
        }
    }
    fn purge(&mut self) {
        match self {
            Self::Legacy(cache) => cache.purge(),
            Self::Budgeted(cache) => cache.purge(),
        }
    }
    fn ticket(&self) -> Option<CacheTicket> {
        match self {
            Self::Legacy(cache) => cache.ticket(),
            Self::Budgeted(cache) => cache.ticket(),
        }
    }
    fn get_shared(&mut self, key: &CacheKey) -> Option<CachedOwner> {
        match self {
            Self::Legacy(cache) => cache.get_shared(key).map(CachedOwner::Legacy),
            Self::Budgeted(cache) => cache.get_shared(key).map(CachedOwner::Budgeted),
        }
    }
    fn insert(
        &mut self,
        ticket: &CacheTicket,
        key: CacheKey,
        body: Vec<u8>,
    ) -> Result<bool, BudgetError> {
        match self {
            Self::Legacy(cache) => Ok(cache.insert(ticket, key, body)),
            Self::Budgeted(cache) => cache.insert(ticket, key, body),
        }
    }
}

/// The complete budgeted handle, not a bare body Arc, owns its entry lease.
/// Bytes::from_owner retains this owner through clones/nonempty slices. Empty
/// slices and Full<Bytes>'s empty-body optimization can release owners sooner;
/// charge lasts only while an actual owner-backed reference remains. Bytes
/// owner boxes, transport buffers and deep copies are outside this accounting.
enum CachedOwner {
    Legacy(Arc<[u8]>),
    Budgeted(BudgetedCachedBody),
}

impl AsRef<[u8]> for CachedOwner {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Legacy(body) => body.as_ref(),
            Self::Budgeted(body) => body.as_bytes(),
        }
    }
}

/// Internal control capability. Never exposed as an unauthenticated HTTP route
/// or a user-supplied capacity flag. A poisoned lock suspends cache use.
#[derive(Clone)]
pub struct PublicCacheControl(Arc<Mutex<CacheBackend>>);

impl PublicCacheControl {
    pub fn status(&self) -> Option<CacheStatus> {
        self.0.lock().ok().map(|cache| cache.status())
    }

    pub fn set_enabled(&self, enabled: bool) -> bool {
        self.update(|cache| cache.set_enabled(enabled))
    }

    /// Local allocation comes from a trusted allocator, not a user setting.
    /// This never changes a shared pool's limit; its caller retains that control.
    pub fn apply_allocation(&self, bytes: usize) -> bool {
        self.update(|cache| cache.apply_budget(bytes))
    }

    pub fn monitoring_lost(&self) -> bool {
        self.update(CacheBackend::monitoring_lost)
    }

    pub fn begin_reconciliation(&self) -> bool {
        self.update(CacheBackend::begin_reconciliation)
    }

    /// Caller must first reconcile every required source with this sealed site.
    /// This does not install a watcher or establish source freshness by itself.
    pub fn reconciliation_completed(&self) -> bool {
        self.update(CacheBackend::reconciliation_completed)
    }

    pub fn purge(&self) -> bool {
        self.update(CacheBackend::purge)
    }

    fn update(&self, change: impl FnOnce(&mut CacheBackend)) -> bool {
        let Ok(mut cache) = self.0.lock() else {
            return false;
        };
        change(&mut cache);
        true
    }
}

#[derive(Clone)]
pub(crate) struct ResponseCache(Arc<Mutex<CacheBackend>>);

pub(crate) fn channel() -> (ResponseCache, PublicCacheControl) {
    channel_with_backend(CacheBackend::Legacy(PageCache::new()))
}

pub(crate) fn budgeted_channel(pool: BudgetPool) -> (ResponseCache, PublicCacheControl) {
    channel_with_backend(CacheBackend::Budgeted(BudgetedPageCache::new(pool)))
}

fn channel_with_backend(backend: CacheBackend) -> (ResponseCache, PublicCacheControl) {
    let cache = Arc::new(Mutex::new(backend));
    (ResponseCache(Arc::clone(&cache)), PublicCacheControl(cache))
}

/// Conservative prototype policy: only the public bundle's GET bodies, with no
/// request-specific state, may use the shared cache. Unknown query names bypass
/// it; names are deliberately not decoded or case folded into the allowlist.
fn eligible(request: &Request, page: &PublicPage) -> bool {
    if request.method() != Method::GET || page.status() != 200 || !request.body().is_end_stream() {
        return false;
    }
    // Unknown headers may carry credentials or select a personalized variant.
    // The immutable bundle does not negotiate representation on these headers.
    if request.headers().keys().any(|name| {
        !matches!(
            name.as_str(),
            "host"
                | "accept"
                | "accept-language"
                | "accept-encoding"
                | "user-agent"
                | "connection"
                | "upgrade-insecure-requests"
                | "sec-fetch-dest"
                | "sec-fetch-mode"
                | "sec-fetch-site"
                | "sec-fetch-user"
                | "sec-ch-ua"
                | "sec-ch-ua-mobile"
                | "sec-ch-ua-platform"
                | "priority"
                | "dnt"
        )
    }) {
        return false;
    }
    request.uri().query().is_none_or(|query| {
        !query.contains(';')
            && query.split('&').all(|part| {
                let name = part.split_once('=').map_or(part, |(name, _)| name);
                matches!(name, "s" | "page" | "paged")
            })
    })
}

/// The sealed page's existing allocation remains owned by Body/Bytes clones and
/// nonempty slices. This adds no response-cache lease: the page allocation, the
/// Bytes owner box and transport buffers remain outside cache accounting. Empty
/// bodies/slices may release the owner early, so lifetime claims require an
/// actual owner-backed reference. This does not revoke bytes already handed out.
fn uncached_body(page: &PublicPage) -> Body {
    Body::from(Bytes::from_owner(page.shared_body()))
}

impl ResponseCache {
    /// The caller has already validated host/target and read metadata from the
    /// sealed generation. Metadata never comes from an independently cached copy.
    pub(crate) fn body(&self, request: &Request, host: &str, page: &PublicPage) -> Body {
        if !eligible(request, page) {
            return uncached_body(page);
        }
        let Ok(mut cache) = self.0.lock() else {
            return uncached_body(page);
        };
        let Some(ticket) = cache.ticket() else {
            return uncached_body(page);
        };
        let key = CacheKey {
            // A separate cache is created for each sealed router instance.
            site: "sealed-startup".into(),
            host: host.into(),
            target: request
                .uri()
                .path_and_query()
                .map_or("/", |p| p.as_str())
                .into(),
            representation: page.content_type().into(),
        };
        if let Some(body) = cache.get_shared(&key) {
            return Body::from(Bytes::from_owner(body));
        }
        // Rejected cache admission is not an HTTP failure or an automatic
        // monitor stop. The fallback borrows the existing sealed-page allocation
        // through a shared owner, with no response-cache charge. A cold-miss
        // cache insertion still makes the explicit Vec copy below.
        if cache.insert(&ticket, key.clone(), page.body().to_vec()) != Ok(true) {
            return uncached_body(page);
        }
        if let Some(body) = cache.get_shared(&key) {
            Body::from(Bytes::from_owner(body))
        } else {
            uncached_body(page)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn direct_data(mut body: Body) -> Bytes {
        // Inspect the actual emitted data frame, rather than a collector that
        // could independently copy or coalesce the bytes under test.
        std::future::poll_fn(|context| std::pin::Pin::new(&mut body).poll_frame(context))
            .await
            .expect("one nonempty data frame")
            .expect("infallible shared body")
            .into_data()
            .expect("data, not trailers")
    }

    #[tokio::test]
    async fn uncached_frame_shares_page_allocation_and_nonempty_slice_keeps_last_owner() {
        let pool = BudgetPool::new(4096);
        let (cache, control) = budgeted_channel(pool.clone());
        let page = PublicPage::new(
            200,
            "text/plain; charset=utf-8".into(),
            b"shared body".to_vec(),
        )
        .unwrap();
        let pointer = page.body().as_ptr();
        let weak = Arc::downgrade(&page.shared_body());
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let body = cache.body(&request, "example.test", &page);
        assert_eq!(pool.stats().unwrap().claimed, 0);
        drop(page);
        drop(cache);
        drop(control);
        assert!(weak.upgrade().is_some());
        let bytes = direct_data(body).await;
        assert_eq!(bytes.as_ptr(), pointer);
        assert_eq!(bytes.as_ref(), b"shared body");
        let slice = bytes.slice(1..6);
        assert_eq!(slice.as_ptr(), pointer.wrapping_add(1));
        drop(bytes);
        assert!(weak.upgrade().is_some());
        assert_eq!(slice.as_ref(), b"hared");
        drop(slice);
        assert!(weak.upgrade().is_none());
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }

    #[test]
    fn an_unpolled_nonempty_fallback_body_retains_and_releases_its_page_owner() {
        let (cache, control) = channel();
        let page = PublicPage::new(
            200,
            "text/plain; charset=utf-8".into(),
            b"unpolled".to_vec(),
        )
        .unwrap();
        let weak = Arc::downgrade(&page.shared_body());
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let body = cache.body(&request, "example.test", &page);
        drop(page);
        drop(cache);
        drop(control);
        assert!(weak.upgrade().is_some());
        drop(body);
        assert!(weak.upgrade().is_none());
    }

    #[tokio::test]
    async fn credential_and_unknown_query_bypasses_use_page_owner_without_reusing_warm_entry() {
        let pool = BudgetPool::new(8192);
        let (cache, control) = budgeted_channel(pool.clone());
        assert!(control.apply_allocation(8192));
        assert!(control.reconciliation_completed());
        let page = PublicPage::new(
            200,
            "text/plain; charset=utf-8".into(),
            b"warm and bypass".to_vec(),
        )
        .unwrap();
        let anonymous = Request::builder().uri("/").body(Body::empty()).unwrap();
        let warm = direct_data(cache.body(&anonymous, "example.test", &page)).await;
        assert_ne!(warm.as_ptr(), page.body().as_ptr());
        let claimed = pool.stats().unwrap().claimed;
        assert!(claimed > 0);
        for request in [
            Request::builder()
                .uri("/")
                .header("authorization", "Bearer private")
                .body(Body::empty())
                .unwrap(),
            Request::builder()
                .uri("/?unknown=value")
                .body(Body::empty())
                .unwrap(),
        ] {
            let bypass = direct_data(cache.body(&request, "example.test", &page)).await;
            assert_eq!(bypass.as_ptr(), page.body().as_ptr());
            assert_ne!(bypass.as_ptr(), warm.as_ptr());
            assert_eq!(bypass.as_ref(), page.body());
            assert_eq!(pool.stats().unwrap().claimed, claimed);
        }
        assert_eq!(control.status().unwrap().entries, 1);
        drop(warm);
        assert!(control.purge());
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }

    #[tokio::test]
    async fn disabled_monitor_loss_local_pool_refusal_and_poison_fallbacks_do_not_charge() {
        for mode in 0..5 {
            let pool = BudgetPool::new(if mode == 3 { 0 } else { 8192 });
            let (cache, control) = budgeted_channel(pool.clone());
            assert!(control.apply_allocation(if mode == 2 { 0 } else { 8192 }));
            assert!(control.reconciliation_completed());
            match mode {
                0 => assert!(control.set_enabled(false)),
                1 => assert!(control.monitoring_lost()),
                4 => {
                    let shared = Arc::clone(&cache.0);
                    assert!(
                        std::thread::spawn(move || {
                            let _guard = shared.lock().unwrap();
                            panic!("simulate cache poison");
                        })
                        .join()
                        .is_err()
                    );
                }
                _ => {}
            }
            let page = PublicPage::new(
                200,
                "text/plain; charset=utf-8".into(),
                b"fallback".to_vec(),
            )
            .unwrap();
            let request = Request::builder().uri("/").body(Body::empty()).unwrap();
            let bytes = direct_data(cache.body(&request, "example.test", &page)).await;
            assert_eq!(bytes.as_ptr(), page.body().as_ptr(), "mode {mode}");
            assert_eq!(bytes.as_ref(), b"fallback");
            assert_eq!(pool.stats().unwrap().claimed, 0, "mode {mode}");
            if mode != 4 {
                assert_eq!(control.status().unwrap().entries, 0);
            }
        }
    }

    #[test]
    fn a_credentialed_request_does_not_reuse_an_already_warm_body() {
        let (cache, control) = channel();
        control.apply_allocation(4096);
        control.reconciliation_completed();
        let page =
            PublicPage::new(200, "text/plain; charset=utf-8".into(), b"public".to_vec()).unwrap();
        let anonymous = Request::builder().uri("/").body(Body::empty()).unwrap();
        drop(cache.body(&anonymous, "example.test", &page));
        let key = CacheKey {
            site: "sealed-startup".into(),
            host: "example.test".into(),
            target: "/".into(),
            representation: page.content_type().into(),
        };
        let CachedOwner::Legacy(stored) = cache.0.lock().unwrap().get_shared(&key).unwrap() else {
            panic!("legacy channel must retain legacy Arc semantics");
        };
        let owners = Arc::strong_count(&stored);
        let hit = cache.body(&anonymous, "example.test", &page);
        assert_eq!(Arc::strong_count(&stored), owners + 1);
        drop(hit);
        let authenticated = Request::builder()
            .uri("/")
            .header("authorization", "Bearer private")
            .body(Body::empty())
            .unwrap();
        let bypass = cache.body(&authenticated, "example.test", &page);
        assert_eq!(Arc::strong_count(&stored), owners);
        drop(bypass);
    }

    #[tokio::test]
    async fn poisoned_cache_falls_back_without_recovering_untrusted_state() {
        let (cache, control) = channel();
        let shared = Arc::clone(&cache.0);
        assert!(
            std::thread::spawn(move || {
                let _guard = shared.lock().unwrap();
                panic!("simulate cache worker failure");
            })
            .join()
            .is_err()
        );
        assert!(control.status().is_none());
        assert!(!control.apply_allocation(4096));
        assert!(!control.reconciliation_completed());
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let page =
            PublicPage::new(200, "text/plain; charset=utf-8".into(), b"public".to_vec()).unwrap();
        let body = cache.body(&request, "example.test", &page);
        assert_eq!(to_bytes(body, 100).await.unwrap(), "public");
    }

    #[test]
    fn budgeted_backend_rejects_stale_ticket_before_zero_pool_admission() {
        let pool = BudgetPool::new(0);
        let (cache, control) = budgeted_channel(pool.clone());
        control.apply_allocation(4096);
        control.reconciliation_completed();
        let old = cache.0.lock().unwrap().ticket().unwrap();
        control.purge();
        let key = CacheKey {
            site: "sealed-startup".into(),
            host: "example.test".into(),
            target: "/".into(),
            representation: "text/plain".into(),
        };
        assert_eq!(
            cache.0.lock().unwrap().insert(&old, key, b"stale".to_vec()),
            Ok(false)
        );
        assert_eq!(control.status().unwrap().entries, 0);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

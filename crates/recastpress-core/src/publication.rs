//! Immutable public generations and a separate publication capability.
//!
//! Only prebuilt anonymous output enters this prototype. Route validation is not
//! WordPress/theme validation. No editable DB or executable code enters a reader.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[path = "publication_redirect.rs"]
mod redirect;
pub use redirect::{OriginFormLocation, RedirectStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrepareError {
    InvalidTarget,
    InvalidContentType,
    InvalidImageSignature,
    InvalidTextAssetEncoding,
    InvalidLocation,
    InvalidStatus,
    DuplicateTarget,
    EmptySite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublishError {
    Conflict,
    Unavailable,
}

/// Shared boundary for prepared routes and the HTTP request adapter.
pub const MAX_TARGET_BYTES: usize = 8192;

/// Exact origin-form target; encoding and query ordering are preserved.
pub fn valid_target(target: &str) -> bool {
    target.len() <= MAX_TARGET_BYTES
        && target.starts_with('/')
        && !target.starts_with("//")
        && !target
            .bytes()
            .any(|b| b <= 0x20 || b == 0x7f || b == b'#' || b == b'\\')
}

// Canonical admitted text MIME references. Text pages and typed redirects select
// the same plain-text binding rather than relying on literal pooling.
static TEXT_HTML: &str = "text/html; charset=utf-8";
static TEXT_PLAIN: &str = "text/plain; charset=utf-8";
static TEXT_RSS: &str = "application/rss+xml; charset=utf-8";
static TEXT_ATOM: &str = "application/atom+xml; charset=utf-8";
static TEXT_CSS: &str = "text/css; charset=utf-8";
static TEXT_JAVASCRIPT: &str = "text/javascript; charset=utf-8";

#[derive(Clone, Debug)]
pub struct PublicPage {
    status: u16,
    // Static MIME storage does not retain the caller String's spare capacity;
    // callers still allocate that input, and dropping it is not an RSS promise.
    content_type: &'static str,
    body: Arc<[u8]>,
    location: Option<OriginFormLocation>,
}

/// Raster MIME/signature subset for explicitly host-prepared public bytes.
/// This is not image decoding, file integrity checking or metadata sanitation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RasterImageKind {
    Png,
    Jpeg,
    WebP,
}

impl RasterImageKind {
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
        }
    }

    fn matches_signature(self, bytes: &[u8]) -> bool {
        // WHATWG MIME Sniffing §6.1 patterns, not its general sniffing algorithm.
        // No leading bytes are ignored and the declared kind is never replaced.
        match self {
            Self::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            Self::Jpeg => bytes.starts_with(b"\xff\xd8\xff"),
            Self::WebP => bytes.starts_with(b"RIFF") && bytes.get(8..14) == Some(b"WEBPVP"),
        }
    }
}

fn validate_status(status: u16) -> Result<(), PrepareError> {
    // Text/raster constructors retain their original status domain.
    if [200, 404, 410].contains(&status) {
        Ok(())
    } else {
        Err(PrepareError::InvalidStatus)
    }
}

impl PublicPage {
    /// Fixed status200 prebuilt stylesheet, not CSS syntax or safety validation.
    /// Exact bytes are retained; no import/URL discovery or external fetch occurs.
    pub fn new_stylesheet(body: Vec<u8>) -> Result<Self, PrepareError> {
        Self::new_text_asset(TEXT_CSS, body)
    }

    /// Fixed status200 prebuilt JavaScript, not script parsing or sandboxing.
    /// This core does not execute it; browser execution is the caller's choice.
    pub fn new_javascript(body: Vec<u8>) -> Result<Self, PrepareError> {
        Self::new_text_asset(TEXT_JAVASCRIPT, body)
    }

    fn new_text_asset(content_type: &'static str, body: Vec<u8>) -> Result<Self, PrepareError> {
        if std::str::from_utf8(&body).is_err() {
            return Err(PrepareError::InvalidTextAssetEncoding);
        }
        Ok(Self {
            status: 200,
            content_type,
            body: body.into(),
            location: None,
        })
    }

    /// Consume an exact text MIME String and select its canonical static value.
    /// The input allocation already exists on entry and is not retained. Page
    /// clones copy this MIME reference; body and Location owners remain shared.
    pub fn new(status: u16, content_type: String, body: Vec<u8>) -> Result<Self, PrepareError> {
        validate_status(status)?;
        let content_type = match content_type.as_str() {
            "text/html; charset=utf-8" => TEXT_HTML,
            "text/plain; charset=utf-8" => TEXT_PLAIN,
            "application/rss+xml; charset=utf-8" => TEXT_RSS,
            "application/atom+xml; charset=utf-8" => TEXT_ATOM,
            _ => return Err(PrepareError::InvalidContentType),
        };
        Ok(Self {
            status,
            content_type,
            body: body.into(),
            location: None,
        })
    }

    /// Explicit binary constructor; the legacy text/feed constructor is unchanged.
    /// Only the declared kind's fixed signature is checked. Truncated/corrupt
    /// images can still match: pixel decoding, complete container/CRC validation,
    /// metadata removal and selection of public content belong to the exporter.
    /// No filesystem, network, PHP or DB access is performed.
    pub fn new_image(
        status: u16,
        kind: RasterImageKind,
        body: Vec<u8>,
    ) -> Result<Self, PrepareError> {
        validate_status(status)?;
        if !kind.matches_signature(&body) {
            return Err(PrepareError::InvalidImageSignature);
        }
        Ok(Self {
            status,
            content_type: kind.content_type(),
            body: body.into(),
            location: None,
        })
    }

    /// Static host-prepared redirect. No request input or destination is fetched.
    pub fn new_redirect(status: RedirectStatus, location: OriginFormLocation) -> Self {
        Self {
            status: status.code(),
            content_type: TEXT_PLAIN,
            body: Vec::new().into(),
            location: Some(location),
        }
    }

    pub fn location(&self) -> Option<&str> {
        self.location.as_ref().map(OriginFormLocation::as_str)
    }

    pub fn status(&self) -> u16 {
        self.status
    }
    pub fn content_type(&self) -> &str {
        self.content_type
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Share the existing immutable payload without copying its bytes. This
    /// owner can retain the complete payload after the page/site is dropped.
    /// Empty payloads have no addressable bytes; adapters may release an empty
    /// owner early. Arc metadata, RSS and downstream transport copies are not
    /// accounted for or eliminated by this accessor.
    pub fn shared_body(&self) -> Arc<[u8]> {
        Arc::clone(&self.body)
    }
}

#[derive(Debug)]
pub struct PreparedSite {
    // One Arc allocation per route at preparation shares page metadata across
    // reads/responses and avoiding nested body/Location Arc clones per read.
    // MIME is static and Location already owns Arc<str>. The per-route Page
    // allocation remains; this is not process/transport allocation accounting.
    pages: BTreeMap<String, Arc<PublicPage>>,
}

impl PreparedSite {
    pub fn targets(&self) -> impl Iterator<Item = &str> {
        self.pages.keys().map(String::as_str)
    }

    pub fn new(
        pages: impl IntoIterator<Item = (String, PublicPage)>,
    ) -> Result<Self, PrepareError> {
        let mut result = BTreeMap::new();
        for (target, page) in pages {
            if !valid_target(&target) {
                return Err(PrepareError::InvalidTarget);
            }
            if result.insert(target, Arc::new(page)).is_some() {
                return Err(PrepareError::DuplicateTarget);
            }
        }
        if result.is_empty() {
            return Err(PrepareError::EmptySite);
        }
        Ok(Self { pages: result })
    }
}

/// Opaque compare-and-swap token, unique across sites and replacements.
#[derive(Clone, Debug)]
pub struct Revision(Arc<()>);

impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Revision {}

#[derive(Debug)]
struct State {
    revision: Revision,
    site: Option<PreparedSite>,
}

/// A read-only public capability. It has no operation for PHP, jobs, or editing.
#[derive(Clone, Debug)]
pub struct PublicReader(Arc<RwLock<State>>);

/// Must be kept in the authenticated management process in a real deployment.
#[derive(Debug)]
pub struct Publisher(Arc<RwLock<State>>);

#[derive(Clone, Debug)]
pub struct PublicResponse {
    revision: Revision,
    // Shared ownership retains this page, not the whole old generation, and
    // does not authorize stale delivery. Body owners can outlive this metadata.
    page: Arc<PublicPage>,
}

impl PublicResponse {
    pub fn revision(&self) -> &Revision {
        &self.revision
    }
    pub fn page(&self) -> &PublicPage {
        self.page.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadError {
    InvalidTarget,
    NotPublished,
    NotFound,
    Stale,
    Unavailable,
}

/// Mandatory caller logical limits, not memory/CPU/transport quotas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionLimits {
    max_pages: usize,
    max_target_bytes: usize,
}
impl SelectionLimits {
    pub fn new(max_pages: usize, max_target_bytes: usize) -> Self {
        Self {
            max_pages,
            max_target_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionError {
    Limits,
    InvalidSelection,
    Allocation,
    Read(ReadError),
}

/// Retains only selected page owners, in caller order including duplicates.
/// This is not a theme context, principal, whole-generation snapshot or delivery.
pub struct PublicSelection {
    revision: Revision,
    pages: Vec<Arc<PublicPage>>,
}
impl std::fmt::Debug for PublicSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicSelection")
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}
impl PublicSelection {
    pub fn revision(&self) -> &Revision {
        &self.revision
    }
    pub fn len(&self) -> usize {
        self.pages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
    pub fn pages(&self) -> impl ExactSizeIterator<Item = &PublicPage> {
        self.pages.iter().map(Arc::as_ref)
    }
    /// Fallible clone bounded by this already accepted selection's page count.
    pub fn try_clone(&self) -> Result<Self, SelectionError> {
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(self.pages.len())
            .map_err(|_| SelectionError::Allocation)?;
        pages.extend(self.pages.iter().map(Arc::clone));
        Ok(Self {
            revision: self.revision.clone(),
            pages,
        })
    }
}

pub fn publication_channel() -> (Publisher, PublicReader) {
    let state = Arc::new(RwLock::new(State {
        revision: Revision(Arc::new(())),
        site: None,
    }));
    (Publisher(Arc::clone(&state)), PublicReader(state))
}

impl Publisher {
    pub fn revision(&self) -> Result<Revision, PublishError> {
        self.0
            .read()
            .map(|s| s.revision.clone())
            .map_err(|_| PublishError::Unavailable)
    }

    /// Replaces the whole generation; partial route updates are not exposed.
    pub fn publish(&self, base: &Revision, site: PreparedSite) -> Result<Revision, PublishError> {
        self.replace(base, Some(site))
    }

    /// Withdraws public output and invalidates every outstanding response token.
    pub fn withdraw(&self, base: &Revision) -> Result<Revision, PublishError> {
        self.replace(base, None)
    }

    fn replace(
        &self,
        base: &Revision,
        site: Option<PreparedSite>,
    ) -> Result<Revision, PublishError> {
        let mut state = self.0.write().map_err(|_| PublishError::Unavailable)?;
        if state.revision != *base {
            return Err(PublishError::Conflict);
        }
        state.revision = Revision(Arc::new(()));
        state.site = site;
        Ok(state.revision.clone())
    }
}

impl PublicReader {
    /// Select all pages under one read lock. Caps/validation/reservation precede
    /// the lock; no read lock survives this call and no unselected page is held.
    pub fn read_selected(
        &self,
        targets: &[&str],
        limits: SelectionLimits,
    ) -> Result<PublicSelection, SelectionError> {
        if targets.is_empty() {
            return Err(SelectionError::InvalidSelection);
        }
        if targets.len() > limits.max_pages {
            return Err(SelectionError::Limits);
        }
        let total = targets
            .iter()
            .try_fold(0_usize, |n, target| n.checked_add(target.len()))
            .ok_or(SelectionError::Limits)?;
        if total > limits.max_target_bytes {
            return Err(SelectionError::Limits);
        }
        if targets.iter().any(|target| !valid_target(target)) {
            return Err(SelectionError::InvalidSelection);
        }
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(targets.len())
            .map_err(|_| SelectionError::Allocation)?;
        let state = self
            .0
            .read()
            .map_err(|_| SelectionError::Read(ReadError::Unavailable))?;
        let site = state
            .site
            .as_ref()
            .ok_or(SelectionError::Read(ReadError::NotPublished))?;
        for target in targets {
            let page = site
                .pages
                .get(*target)
                .ok_or(SelectionError::Read(ReadError::NotFound))?;
            pages.push(Arc::clone(page));
        }
        Ok(PublicSelection {
            revision: state.revision.clone(),
            pages,
        })
    }

    /// Bounded synchronous group delivery with one current-revision check.
    /// Do not reenter this channel (including PublicReader) or wait for
    /// publication: a pending writer can make recursive reads deadlock. Do not
    /// defer transport or perform unbounded work. Already delivered bytes cannot
    /// be recalled.
    pub fn deliver_selected<T>(
        &self,
        selected: &PublicSelection,
        send: impl FnOnce(&PublicSelection) -> T,
    ) -> Result<T, ReadError> {
        let state = self.0.read().map_err(|_| ReadError::Unavailable)?;
        if state.site.is_none() || state.revision != selected.revision {
            return Err(ReadError::Stale);
        }
        Ok(send(selected))
    }

    pub fn read(&self, target: &str) -> Result<PublicResponse, ReadError> {
        if !valid_target(target) {
            return Err(ReadError::InvalidTarget);
        }
        let state = self.0.read().map_err(|_| ReadError::Unavailable)?;
        let site = state.site.as_ref().ok_or(ReadError::NotPublished)?;
        let page = site.pages.get(target).ok_or(ReadError::NotFound)?;
        Ok(PublicResponse {
            revision: state.revision.clone(),
            page: Arc::clone(page),
        })
    }

    /// Runs a bounded synchronous delivery while holding the generation read lock.
    /// A withdrawal cannot complete between validation and delivery. The callback
    /// must not reenter Publisher, spawn deferred delivery, or block indefinitely.
    /// Bytes already sent before a withdrawal cannot be recalled.
    pub fn deliver<T>(
        &self,
        response: &PublicResponse,
        send: impl FnOnce(&PublicPage) -> T,
    ) -> Result<T, ReadError> {
        let state = self.0.read().map_err(|_| ReadError::Unavailable)?;
        if state.site.is_none() || state.revision != response.revision {
            return Err(ReadError::Stale);
        }
        Ok(send(&response.page))
    }
}

#[cfg(test)]
mod page_owner_tests {
    use super::*;

    fn page(body: &[u8]) -> PublicPage {
        PublicPage::new(200, "text/plain; charset=utf-8".into(), body.to_vec()).unwrap()
    }

    #[test]
    fn replacement_releases_unselected_pages_but_response_clones_keep_the_selected_page() {
        let old_site = PreparedSite::new([
            ("/selected".into(), page(b"selected old bytes")),
            ("/other".into(), page(b"other old bytes")),
        ])
        .unwrap();
        let selected = Arc::downgrade(old_site.pages.get("/selected").unwrap());
        let other = Arc::downgrade(old_site.pages.get("/other").unwrap());
        let (publisher, reader) = publication_channel();
        let first = publisher
            .publish(&publisher.revision().unwrap(), old_site)
            .unwrap();
        let old = reader.read("/selected").unwrap();
        let retained = old.clone();
        let second = publisher
            .publish(
                &first,
                PreparedSite::new([("/selected".into(), page(b"new bytes"))]).unwrap(),
            )
            .unwrap();
        assert!(other.upgrade().is_none());
        assert!(selected.upgrade().is_some());
        assert_eq!(
            reader.deliver(&old, |_| panic!("stale delivery ran")),
            Err(ReadError::Stale)
        );
        assert_eq!(old.page().content_type(), "text/plain; charset=utf-8");
        assert_eq!(old.page().body(), b"selected old bytes");
        drop(old);
        assert!(selected.upgrade().is_some());
        publisher.withdraw(&second).unwrap();
        assert_eq!(
            reader.deliver(&retained, |_| panic!("withdrawn delivery ran")),
            Err(ReadError::Stale)
        );
        drop(retained);
        assert!(selected.upgrade().is_none());
    }

    #[test]
    fn retained_body_bytes_do_not_pin_page_metadata_after_withdrawal_and_last_response_drop() {
        let site = PreparedSite::new([("/".into(), page(b"body owner survives"))]).unwrap();
        let metadata = Arc::downgrade(site.pages.get("/").unwrap());
        let (publisher, reader) = publication_channel();
        let revision = publisher
            .publish(&publisher.revision().unwrap(), site)
            .unwrap();
        let response = reader.read("/").unwrap();
        let body = response.page().shared_body();
        let weak_body = Arc::downgrade(&body);
        publisher.withdraw(&revision).unwrap();
        assert!(metadata.upgrade().is_some());
        drop(response);
        assert!(metadata.upgrade().is_none());
        assert!(weak_body.upgrade().is_some());
        assert_eq!(&*body, b"body owner survives");
        drop(body);
        assert!(weak_body.upgrade().is_none());
    }
}

#[cfg(test)]
#[path = "selected_pages_contract.rs"]
mod selected_pages_contract;

#[cfg(test)]
#[path = "sealed_assets_contract.rs"]
mod sealed_assets_contract;

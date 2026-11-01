//! Experimental explicit-body bundles, not the future Wasm package format.
//! V1 retains UTF-8 JSON bodies. V2 uses required hex bodies for text and raster
//! signatures. V3 adds fixed UTF-8 CSS/JavaScript assets with report schema 2.
//! No version resolves page filenames, URLs or other sources.

use recastpress_core::publication::{
    OriginFormLocation, PreparedSite, PublicPage, RasterImageKind, RedirectStatus,
};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

pub const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024;

/// Experimental check report schema, distinct from the accepted bundle version.
/// No target, Location, path or body content is retained in this report.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct BundleCheckReport {
    format_version: u32,
    accepted_bundle_version: u32,
    pages: usize,
    redirects: usize,
    body_bytes: usize,
    // Body-page counts only; redirects have no body and are counted separately.
    mime_counts: BTreeMap<&'static str, usize>,
}

struct Admission {
    site: PreparedSite,
    report: BundleCheckReport,
}

const BODY_MIMES: [&str; 7] = [
    "text/html; charset=utf-8",
    "text/plain; charset=utf-8",
    "application/rss+xml; charset=utf-8",
    "application/atom+xml; charset=utf-8",
    "image/png",
    "image/jpeg",
    "image/webp",
];

const BODY_MIMES_V3: [&str; 9] = [
    "text/html; charset=utf-8",
    "text/plain; charset=utf-8",
    "application/rss+xml; charset=utf-8",
    "application/atom+xml; charset=utf-8",
    "image/png",
    "image/jpeg",
    "image/webp",
    "text/css; charset=utf-8",
    "text/javascript; charset=utf-8",
];

fn finish_admission(pages: Vec<(String, PublicPage)>, version: u32) -> Result<Admission, String> {
    let mimes: &[&'static str] = if version == 3 {
        &BODY_MIMES_V3
    } else {
        &BODY_MIMES
    };
    let mut report = BundleCheckReport {
        format_version: if version == 3 { 2 } else { 1 },
        accepted_bundle_version: version,
        pages: 0,
        redirects: 0,
        body_bytes: 0,
        mime_counts: mimes.iter().copied().map(|mime| (mime, 0)).collect(),
    };
    for (_, page) in &pages {
        report.pages = report
            .pages
            .checked_add(1)
            .ok_or("bundle summary count overflow")?;
        report.body_bytes = report
            .body_bytes
            .checked_add(page.body().len())
            .ok_or("bundle summary byte overflow")?;
        if page.location().is_some() {
            report.redirects = report
                .redirects
                .checked_add(1)
                .ok_or("bundle summary count overflow")?;
        } else {
            let count = report
                .mime_counts
                .get_mut(page.content_type())
                .ok_or("unsupported admitted bundle MIME")?;
            *count = count
                .checked_add(1)
                .ok_or("bundle summary count overflow")?;
        }
    }
    // A report is available only after all generation admission, including
    // nonempty/unique valid targets. No publication channel is involved.
    let site = PreparedSite::new(pages).map_err(|e| format!("invalid public generation: {e:?}"))?;
    Ok(Admission { site, report })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionEnvelope {
    format_version: u32,
    #[serde(rename = "pages")]
    _pages: IgnoredAny,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleV1 {
    #[serde(rename = "format_version")]
    _format_version: u32,
    pages: Vec<Page>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    target: String,
    status: u16,
    content_type: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleV2 {
    #[serde(rename = "format_version")]
    _format_version: u32,
    pages: Vec<PageV2>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PageV2 {
    Body(HexPage),
    Redirect(RedirectPage),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RedirectPage {
    target: String,
    status: u16,
    location: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HexPage {
    target: String,
    status: u16,
    content_type: String,
    body_hex: String,
}

pub fn load(path: &Path) -> Result<PreparedSite, String> {
    load_admission(path).map(|admission| admission.site)
}

/// Run exactly the existing v1/v2 or v3 asset admission and return only a content-free
/// development report. Reads/allocates the bundle but never publishes or fetches.
/// This does not validate complete images/XML, redirect graphs or public policy.
pub fn check(path: &Path) -> Result<BundleCheckReport, String> {
    check_payload(&read_payload(path)?)
}

/// Share the complete existing admission with receipt-verified, already-read
/// bytes. This report never contains a route, body, digest or input path.
pub(crate) fn check_payload(bytes: &[u8]) -> Result<BundleCheckReport, String> {
    parse_admission(bytes).map(|admission| admission.report)
}

fn load_admission(path: &Path) -> Result<Admission, String> {
    let bytes = read_payload(path)?;
    parse_admission(&bytes)
}

/// Shared single-open, bounded payload read. The extra byte is a sentinel;
/// admission (including receipt verification) rejects any oversized buffer.
pub(crate) fn read_payload(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("cannot open bundle: {e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_BUNDLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read bundle: {e}"))?;
    Ok(bytes)
}

pub fn parse(bytes: &[u8]) -> Result<PreparedSite, String> {
    parse_admission(bytes).map(|admission| admission.site)
}

fn parse_admission(bytes: &[u8]) -> Result<Admission, String> {
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err("bundle exceeds 8 MiB".into());
    }
    // Do not select a version through a generic JSON map: it would discard
    // duplicate keys before the required typed structures can reject them.
    let envelope: VersionEnvelope =
        serde_json::from_slice(bytes).map_err(|e| format!("invalid bundle JSON: {e}"))?;
    match envelope.format_version {
        1 => parse_v1(bytes),
        2 => parse_v2(bytes),
        3 => parse_hex_version(bytes, 3),
        _ => Err("unsupported bundle format version".into()),
    }
}

fn checked_target(target: &str) -> Result<(), String> {
    if matches!(
        target.split('?').next(),
        Some("/health/live" | "/health/ready")
    ) {
        return Err("public page conflicts with reserved health route".into());
    }
    Ok(())
}

fn parse_v1(bytes: &[u8]) -> Result<Admission, String> {
    let bundle: BundleV1 =
        serde_json::from_slice(bytes).map_err(|e| format!("invalid bundle JSON: {e}"))?;
    let pages = bundle
        .pages
        .into_iter()
        .map(|p| {
            checked_target(&p.target)?;
            PublicPage::new(p.status, p.content_type, p.body.into_bytes())
                .map(|page| (p.target, page))
                .map_err(|e| format!("invalid public page: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    finish_admission(pages, 1)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_v2(bytes: &[u8]) -> Result<Admission, String> {
    parse_hex_version(bytes, 2)
}

// V2 keeps its original MIME domain; V3 explicitly selects the asset extension.
fn parse_hex_version(bytes: &[u8], version: u32) -> Result<Admission, String> {
    let bundle: BundleV2 =
        serde_json::from_slice(bytes).map_err(|e| format!("invalid bundle JSON: {e}"))?;
    let mut total = 0usize;
    // Validate every body's complete wire and the aggregate before any decoded
    // body allocation. JSON strings/input/Arc conversion are not RSS-budgeted.
    for page in &bundle.pages {
        let page = match page {
            PageV2::Body(page) => page,
            PageV2::Redirect(page) => {
                checked_target(&page.target)?;
                continue;
            }
        };
        checked_target(&page.target)?;
        if page.body_hex.len() % 2 != 0
            || !page.body_hex.bytes().all(|byte| hex_nibble(byte).is_some())
        {
            return Err("bundle body_hex requires pairs of ASCII hex digits".into());
        }
        total = total
            .checked_add(page.body_hex.len() / 2)
            .filter(|size| *size <= bytes.len() && (*size as u64) <= MAX_BUNDLE_BYTES)
            .ok_or("decoded bundle bodies exceed the aggregate byte bound")?;
    }
    let pages = bundle
        .pages
        .into_iter()
        .map(|page| {
            let page = match page {
                PageV2::Body(page) => page,
                PageV2::Redirect(page) => {
                    let status = RedirectStatus::try_from(page.status)
                        .map_err(|e| format!("invalid redirect status: {e:?}"))?;
                    let location = OriginFormLocation::new(page.location)
                        .map_err(|e| format!("invalid redirect Location: {e:?}"))?;
                    return Ok((page.target, PublicPage::new_redirect(status, location)));
                }
            };
            let mut body = Vec::new();
            body.try_reserve_exact(page.body_hex.len() / 2)
                .map_err(|_| "cannot allocate decoded bundle body")?;
            for pair in page.body_hex.as_bytes().chunks_exact(2) {
                // Whole-wire admission above excludes all invalid nibbles.
                let high = hex_nibble(pair[0]).ok_or("invalid admitted hex body")?;
                let low = hex_nibble(pair[1]).ok_or("invalid admitted hex body")?;
                body.push((high << 4) | low);
            }
            let image = match page.content_type.as_str() {
                "image/png" => Some(RasterImageKind::Png),
                "image/jpeg" => Some(RasterImageKind::Jpeg),
                "image/webp" => Some(RasterImageKind::WebP),
                _ => None,
            };
            let prepared = if let Some(kind) = image {
                PublicPage::new_image(page.status, kind, body)
            } else if version == 3
                && matches!(
                    page.content_type.as_str(),
                    "text/css; charset=utf-8" | "text/javascript; charset=utf-8"
                )
            {
                if page.status != 200 {
                    return Err("prepared text asset requires status 200".into());
                }
                match page.content_type.as_str() {
                    "text/css; charset=utf-8" => PublicPage::new_stylesheet(body),
                    _ => PublicPage::new_javascript(body),
                }
            } else {
                if std::str::from_utf8(&body).is_err() {
                    return Err("text/feed bundle body is not valid UTF-8".into());
                }
                PublicPage::new(page.status, page.content_type, body)
            };
            prepared
                .map(|prepared| (page.target, prepared))
                .map_err(|e| format!("invalid public page: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    finish_admission(pages, version)
}

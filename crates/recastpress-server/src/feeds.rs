//! Pure RSS 2.0 / Atom 1.0 serializer prototype for host-prepared public data.
//!
//! Sources: https://www.rssboard.org/rss-specification,
//! https://www.rfc-editor.org/rfc/rfc4287.html, https://www.w3.org/TR/xml/.
//! This is a candidate subset: HTTP(S) ASCII URL identifiers only, UTC whole
//! seconds in 1900..=9999, nonempty metadata and unique entry identifiers.
//! URL syntax additionally follows https://www.rfc-editor.org/rfc/rfc3986.html;
//! explicit ports must fit u16, and IPvFuture / scoped IPv6 are not supported.
//! Duplicate IDs use literal string equality, without URL normalization,
//! host resolution or destination authorization.
//! It does not select public posts, remove private data, sanitize summary HTML,
//! execute WP filters or continuously publish feeds. The comments submodule
//! separately serializes caller-prepared comment data without public selection.
//! The caller must supply HTML already prepared for public syndication.

#[path = "comment_feeds.rs"]
pub mod comments;

use axum::http::Uri;
use recastpress_core::resource_budget::{BudgetError, BudgetPool, Reservation, ResourceKind};
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UtcTimestamp {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl UtcTimestamp {
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<Self, FeedError> {
        if !(1900..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err(FeedError::InvalidTimestamp);
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    pub fn atom_text(self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    pub fn rss_text(self) -> String {
        const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let year = u32::from(self.year);
        let leap_days_before = |year: u32| (year - 1) / 4 - (year - 1) / 100 + (year - 1) / 400;
        let mut days = (year - 1900) * 365 + leap_days_before(year) - leap_days_before(1900);
        for month in 1..self.month {
            days += u32::from(days_in_month(self.year, month));
        }
        days += u32::from(self.day) - 1;
        // 1900-01-01 was Monday. No platform timezone/locale APIs are involved.
        let weekday = WEEKDAYS[((days + 1) % 7) as usize];
        format!(
            "{weekday}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
            self.day,
            MONTHS[usize::from(self.month - 1)],
            self.year,
            self.hour,
            self.minute,
            self.second
        )
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedEntry {
    pub id: String,
    pub title: String,
    pub link: String,
    pub summary_html: String,
    pub author_name: String,
    pub published: UtcTimestamp,
    pub updated: UtcTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedDocument {
    pub id: String,
    pub title: String,
    pub home_url: String,
    pub self_url: String,
    pub description: String,
    pub author_name: String,
    pub updated: UtcTimestamp,
    pub entries: Vec<FeedEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedFormat {
    Rss2,
    Atom1,
}

impl FeedFormat {
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Rss2 => "application/rss+xml; charset=utf-8",
            Self::Atom1 => "application/atom+xml; charset=utf-8",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeedError {
    InvalidTimestamp,
    MissingField(&'static str),
    InvalidXmlCharacter(&'static str),
    InvalidUrl(&'static str),
    DuplicateEntryId,
    OutputLimitExceeded,
}

impl fmt::Display for FeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimestamp => formatter.write_str("invalid candidate UTC feed timestamp"),
            Self::MissingField(field) => write!(formatter, "missing required feed field: {field}"),
            Self::InvalidXmlCharacter(field) => {
                write!(formatter, "XML 1.0 forbids a character in: {field}")
            }
            Self::InvalidUrl(field) => write!(
                formatter,
                "feed field requires an ASCII HTTP(S) URL without userinfo: {field}"
            ),
            Self::DuplicateEntryId => formatter.write_str("duplicate entry identifier"),
            Self::OutputLimitExceeded => {
                formatter.write_str("feed exceeds caller output byte limit")
            }
        }
    }
}
impl std::error::Error for FeedError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BudgetedFeedError {
    Feed(FeedError),
    Budget(BudgetError),
    AllocationFailed,
}

impl fmt::Display for BudgetedFeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Feed(error) => error.fmt(formatter),
            Self::Budget(BudgetError::Exhausted) => {
                formatter.write_str("feed logical budget is exhausted")
            }
            Self::Budget(BudgetError::Unavailable) => {
                formatter.write_str("feed logical budget is unavailable")
            }
            Self::AllocationFailed => formatter.write_str("feed output allocation failed"),
        }
    }
}

impl std::error::Error for BudgetedFeedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Feed(error) => Some(error),
            _ => None,
        }
    }
}

/// Move-only feed bytes and their logical lease. Field drop order destroys the
/// bytes before returning credit. Arc sharing keeps one lease until last drop.
/// No owned/mutable buffer escape is provided; explicit caller copies of the
/// borrowed bytes are outside this accounting.
pub struct BudgetedFeedOutput {
    bytes: Vec<u8>,
    format: FeedFormat,
    lease: Reservation,
}

impl BudgetedFeedOutput {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn format(&self) -> FeedFormat {
        self.format
    }

    pub fn charged_bytes(&self) -> usize {
        self.lease.bytes()
    }
}

impl fmt::Debug for BudgetedFeedOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BudgetedFeedOutput")
            .field("output_bytes", &self.bytes.len())
            .field("format", &self.format)
            .field("charged_bytes", &self.charged_bytes())
            .finish_non_exhaustive()
    }
}

/// Returns a complete document or an error, never a partially rendered prefix.
/// max_bytes caps output bytes, not input memory or all temporary allocations;
/// it is an internal caller constraint, not a product capacity setting.
pub fn render(
    document: &FeedDocument,
    format: FeedFormat,
    max_bytes: usize,
) -> Result<Vec<u8>, FeedError> {
    validate(document)?;
    let mut output = XmlWriter::new(max_bytes, true);
    render_document(&mut output, document, format)?;
    Ok(output.bytes)
}

/// Validate and count the complete escaped XML before reserving Rendering, then
/// allocate and serialize using the same traversal. The caller's max_bytes caps
/// XML bytes only; the pool charge adds size_of::<BudgetedFeedOutput>() for this
/// owner's Vec/format/lease header. Allocation failure returns no partial output.
///
/// Input documents, validation BTreeSet/URI work, timestamp strings and other
/// intermediates, Vec capacity/allocator rounding, Arc wrapper nodes, peak/RSS
/// and other copies are unpooled. No automatic allowance, cross-process budget,
/// CLI/public/cache connection or cache-stop policy is introduced.
/// Reservation drop follows the pool's conservative poison policy; an
/// unavailable poisoned ledger is not recovered to advertise released credit.
pub fn render_budgeted(
    document: &FeedDocument,
    format: FeedFormat,
    max_bytes: usize,
    pool: BudgetPool,
) -> Result<BudgetedFeedOutput, BudgetedFeedError> {
    validate(document).map_err(BudgetedFeedError::Feed)?;
    let mut counter = XmlWriter::new(max_bytes, false);
    render_document(&mut counter, document, format).map_err(BudgetedFeedError::Feed)?;
    build_budgeted_output(format, counter.length, pool, |output| {
        render_document(output, document, format)
    })
}

fn build_budgeted_output(
    format: FeedFormat,
    length: usize,
    pool: BudgetPool,
    write: impl FnOnce(&mut XmlWriter) -> Result<(), FeedError>,
) -> Result<BudgetedFeedOutput, BudgetedFeedError> {
    let charge = length
        .checked_add(std::mem::size_of::<BudgetedFeedOutput>())
        .ok_or(BudgetedFeedError::Budget(BudgetError::Exhausted))?;
    let lease = pool
        .try_reserve(ResourceKind::Rendering, charge)
        .map_err(BudgetedFeedError::Budget)?;
    // This is the first allocation of the final output. On failure/unwind,
    // the later-declared writer is destroyed before the reservation is dropped.
    let mut output = XmlWriter::new(length, true);
    output
        .bytes
        .try_reserve_exact(length)
        .map_err(|_| BudgetedFeedError::AllocationFailed)?;
    // No pool lock is held across this private callback. Tests can inject a
    // deterministic serialization failure after allocation/reservation.
    write(&mut output).map_err(BudgetedFeedError::Feed)?;
    if output.length != length {
        return Err(BudgetedFeedError::Feed(FeedError::OutputLimitExceeded));
    }
    Ok(BudgetedFeedOutput {
        bytes: output.bytes,
        format,
        lease,
    })
}

fn render_document(
    output: &mut XmlWriter,
    document: &FeedDocument,
    format: FeedFormat,
) -> Result<(), FeedError> {
    output.push("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    match format {
        FeedFormat::Rss2 => render_rss(output, document),
        FeedFormat::Atom1 => render_atom(output, document),
    }
}

fn text(value: &str, field: &'static str, required: bool) -> Result<(), FeedError> {
    if required && value.trim().is_empty() {
        return Err(FeedError::MissingField(field));
    }
    if !value.chars().all(|ch| matches!(ch as u32, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)) {
        return Err(FeedError::InvalidXmlCharacter(field));
    }
    Ok(())
}

fn url(value: &str, field: &'static str) -> Result<(), FeedError> {
    text(value, field, true)?;
    if !value.is_ascii() {
        return Err(FeedError::InvalidUrl(field));
    }
    let uri: Uri = value.parse().map_err(|_| FeedError::InvalidUrl(field))?;
    if !uri.scheme_str().is_some_and(|scheme| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    }) || uri
        .authority()
        .is_none_or(|authority| authority.host().is_empty() || authority.as_str().contains('@'))
    {
        return Err(FeedError::InvalidUrl(field));
    }
    // http::Uri also accepts malformed escapes/ports and drops fragments.
    // Check original components, then serialize the original URL unchanged.
    let (_, after_scheme) = value
        .split_once("://")
        .ok_or(FeedError::InvalidUrl(field))?;
    let end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    if !valid_authority(&after_scheme[..end]) {
        return Err(FeedError::InvalidUrl(field));
    }
    let tail = &after_scheme[end..];
    let (before_fragment, fragment) = tail
        .split_once('#')
        .map_or((tail, None), |(a, b)| (a, Some(b)));
    let (path, query) = before_fragment
        .split_once('?')
        .map_or((before_fragment, None), |(a, b)| (a, Some(b)));
    if !valid_uri_bytes(path, |byte| {
        uri_character(byte) || matches!(byte, b':' | b'@' | b'/')
    }) || query.is_some_and(|query| !valid_uri_bytes(query, query_character))
        || fragment.is_some_and(|fragment| !valid_uri_bytes(fragment, query_character))
    {
        return Err(FeedError::InvalidUrl(field));
    }
    Ok(())
}

fn uri_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
        )
}

fn query_character(byte: u8) -> bool {
    uri_character(byte) || matches!(byte, b':' | b'@' | b'/' | b'?')
}

fn valid_uri_bytes(value: &str, allowed: impl Fn(u8) -> bool) -> bool {
    let bytes = value.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'%' {
            if !bytes
                .get(cursor + 1..cursor + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
            {
                return false;
            }
            cursor += 3;
        } else {
            if !allowed(bytes[cursor]) {
                return false;
            }
            cursor += 1;
        }
    }
    true
}

fn valid_authority(value: &str) -> bool {
    let (valid_host, port) = if let Some(bracketed) = value.strip_prefix('[') {
        let Some((host, tail)) = bracketed.split_once(']') else {
            return false;
        };
        let port = if tail.is_empty() {
            None
        } else if let Some(port) = tail.strip_prefix(':') {
            Some(port)
        } else {
            return false;
        };
        (host.parse::<std::net::Ipv6Addr>().is_ok(), port)
    } else {
        let (host, port) = value
            .split_once(':')
            .map_or((value, None), |(host, port)| (host, Some(port)));
        (
            !host.is_empty() && valid_uri_bytes(host, uri_character),
            port,
        )
    };
    valid_host
        && port.is_none_or(|port| {
            !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit())
                && port.parse::<u16>().is_ok()
        })
}

fn validate(document: &FeedDocument) -> Result<(), FeedError> {
    url(&document.id, "feed.id")?;
    url(&document.home_url, "feed.home_url")?;
    url(&document.self_url, "feed.self_url")?;
    text(&document.title, "feed.title", true)?;
    text(&document.description, "feed.description", true)?;
    text(&document.author_name, "feed.author_name", true)?;
    let mut identifiers = BTreeSet::new();
    for entry in &document.entries {
        url(&entry.id, "entry.id")?;
        url(&entry.link, "entry.link")?;
        text(&entry.title, "entry.title", true)?;
        text(&entry.author_name, "entry.author_name", true)?;
        text(&entry.summary_html, "entry.summary_html", false)?;
        if !identifiers.insert(entry.id.as_str()) {
            return Err(FeedError::DuplicateEntryId);
        }
    }
    Ok(())
}

struct XmlWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
    length: usize,
    write: bool,
}
impl XmlWriter {
    fn new(max_bytes: usize, write: bool) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
            length: 0,
            write,
        }
    }

    fn push(&mut self, value: &str) -> Result<(), FeedError> {
        let size = self
            .length
            .checked_add(value.len())
            .ok_or(FeedError::OutputLimitExceeded)?;
        if size > self.max_bytes {
            return Err(FeedError::OutputLimitExceeded);
        }
        if self.write {
            self.bytes.extend_from_slice(value.as_bytes());
        }
        self.length = size;
        Ok(())
    }

    fn escaped(&mut self, value: &str, attribute: bool) -> Result<(), FeedError> {
        for ch in value.chars() {
            match ch {
                '&' => self.push("&amp;")?,
                '<' => self.push("&lt;")?,
                '>' => self.push("&gt;")?,
                '"' => self.push("&quot;")?,
                '\'' => self.push("&apos;")?,
                '\r' => self.push("&#13;")?,
                '\n' if attribute => self.push("&#10;")?,
                '\t' if attribute => self.push("&#9;")?,
                _ => {
                    let mut buffer = [0; 4];
                    self.push(ch.encode_utf8(&mut buffer))?;
                }
            }
        }
        Ok(())
    }

    fn element(&mut self, name: &str, value: &str) -> Result<(), FeedError> {
        self.push("<")?;
        self.push(name)?;
        self.push(">")?;
        self.escaped(value, false)?;
        self.push("</")?;
        self.push(name)?;
        self.push(">\n")
    }

    fn author(&mut self, value: &str) -> Result<(), FeedError> {
        self.push("<author>")?;
        self.element("name", value)?;
        self.push("</author>\n")
    }

    fn link(
        &mut self,
        tag: &str,
        relation: &str,
        href: &str,
        kind: Option<&str>,
    ) -> Result<(), FeedError> {
        self.push("<")?;
        self.push(tag)?;
        self.push(" rel=\"")?;
        self.escaped(relation, true)?;
        self.push("\" href=\"")?;
        self.escaped(href, true)?;
        self.push("\"")?;
        if let Some(kind) = kind {
            self.push(" type=\"")?;
            self.escaped(kind, true)?;
            self.push("\"")?;
        }
        self.push("/>\n")
    }
}

fn render_rss(output: &mut XmlWriter, document: &FeedDocument) -> Result<(), FeedError> {
    output.push("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><channel>\n")?;
    output.element("title", &document.title)?;
    output.element("link", &document.home_url)?;
    output.element("description", &document.description)?;
    output.link(
        "atom:link",
        "self",
        &document.self_url,
        Some(FeedFormat::Rss2.content_type()),
    )?;
    output.element("dc:creator", &document.author_name)?;
    output.element("lastBuildDate", &document.updated.rss_text())?;
    for entry in &document.entries {
        output.push("<item>\n")?;
        output.element("title", &entry.title)?;
        output.element("link", &entry.link)?;
        output.push("<guid isPermaLink=\"false\">")?;
        output.escaped(&entry.id, false)?;
        output.push("</guid>\n")?;
        output.element("description", &entry.summary_html)?;
        output.element("dc:creator", &entry.author_name)?;
        output.element("pubDate", &entry.published.rss_text())?;
        output.element("atom:updated", &entry.updated.atom_text())?;
        output.push("</item>\n")?;
    }
    output.push("</channel></rss>\n")
}

fn render_atom(output: &mut XmlWriter, document: &FeedDocument) -> Result<(), FeedError> {
    output.push("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n")?;
    output.element("id", &document.id)?;
    output.element("title", &document.title)?;
    output.element("updated", &document.updated.atom_text())?;
    output.author(&document.author_name)?;
    output.element("subtitle", &document.description)?;
    output.link(
        "link",
        "self",
        &document.self_url,
        Some(FeedFormat::Atom1.content_type()),
    )?;
    output.link("link", "alternate", &document.home_url, None)?;
    for entry in &document.entries {
        output.push("<entry>\n")?;
        output.element("id", &entry.id)?;
        output.element("title", &entry.title)?;
        output.element("updated", &entry.updated.atom_text())?;
        output.element("published", &entry.published.atom_text())?;
        output.author(&entry.author_name)?;
        output.link("link", "alternate", &entry.link, None)?;
        output.push("<summary type=\"html\">")?;
        output.escaped(&entry.summary_html, false)?;
        output.push("</summary>\n")?;
        output.push("</entry>\n")?;
    }
    output.push("</feed>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attributes_encode_cr_lf_and_tab_without_xml_whitespace_normalization() {
        let mut output = XmlWriter::new(usize::MAX, true);
        output.escaped("&<>\"'\r\n\t日", true).unwrap();
        assert_eq!(
            String::from_utf8(output.bytes).unwrap(),
            "&amp;&lt;&gt;&quot;&apos;&#13;&#10;&#9;日"
        );
    }

    #[test]
    fn failed_write_after_reservation_releases_only_its_lease_and_returns_no_output() {
        let pool = BudgetPool::new(usize::MAX);
        let other = pool.try_reserve(ResourceKind::InternalRows, 7).unwrap();
        let charge = 10 + std::mem::size_of::<BudgetedFeedOutput>();
        let result = build_budgeted_output(FeedFormat::Atom1, 10, pool.clone(), |output| {
            assert_eq!(
                pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
                charge
            );
            output.push("partial")?;
            Err(FeedError::OutputLimitExceeded)
        });
        assert!(matches!(
            result,
            Err(BudgetedFeedError::Feed(FeedError::OutputLimitExceeded))
        ));
        assert_eq!(
            pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
            0
        );
        assert_eq!(pool.stats().unwrap().claimed, 7);
        drop(other);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }

    #[test]
    fn impossible_vector_capacity_releases_rendering_lease_and_checked_charge_never_wraps() {
        let pool = BudgetPool::new(usize::MAX);
        let other = pool.try_reserve(ResourceKind::InternalRows, 7).unwrap();
        let impossible_capacity = (isize::MAX as usize).checked_add(1).unwrap();
        // Vec<u8> rejects capacity > isize::MAX without attempting a huge
        // allocation. The logical reservation itself fits the usize pool.
        let result =
            build_budgeted_output(FeedFormat::Atom1, impossible_capacity, pool.clone(), |_| {
                panic!("serialization must not run after allocation refusal")
            });
        assert!(matches!(result, Err(BudgetedFeedError::AllocationFailed)));
        assert_eq!(
            pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
            0
        );
        assert_eq!(pool.stats().unwrap().claimed, 7);
        let overflow = build_budgeted_output(FeedFormat::Atom1, usize::MAX, pool.clone(), |_| {
            panic!("serialization must not run after charge overflow")
        });
        assert!(matches!(
            overflow,
            Err(BudgetedFeedError::Budget(BudgetError::Exhausted))
        ));
        assert_eq!(pool.stats().unwrap().claimed, 7);
        drop(other);
        assert_eq!(pool.stats().unwrap().claimed, 0);
    }
}

//! Explicit request-header hints from pinned WordPress 7.1.3 `load.php`.
//! No server globals, body parsing, HTTP authorization or product routing.

use crate::wp_is_json_media_type_c_bytes;

const XML_TYPES: [&[u8]; 6] = [
    b"text/xml",
    b"application/rss+xml",
    b"application/atom+xml",
    b"application/rdf+xml",
    b"text/xml+oembed",
    b"application/xml+oembed",
];

/// WordPress `wp_is_json_request` (`load.php:1917`) for explicit optional byte
/// headers under the C locale. Accept is tested first; Content-Type is tested
/// only if Accept does not match. `None` models an absent or null `$_SERVER`
/// field (`isset=false`), and an empty byte string remains a present field.
///
/// Stateless, allocation-free and read-only. Non-string PHP inputs, other
/// server context, non-C regex semantics, memoization and PCRE failures are
/// unmodeled. This is only WordPress's hint, ignoring q values; it does not
/// parse JSON bodies, authorize requests or connect to product HTTP handling.
pub fn wp_is_json_request_c_bytes(accept: Option<&[u8]>, content_type: Option<&[u8]>) -> bool {
    accept.is_some_and(wp_is_json_media_type_c_bytes)
        || content_type.is_some_and(wp_is_json_media_type_c_bytes)
}

/// WordPress `wp_is_xml_request` (`load.php:1982`) for explicit optional byte
/// headers. Accept uses case-sensitive substring search for six fixed types;
/// Content-Type uses exact membership in that set. `application/xml` alone is
/// not accepted. `None` models absent/null (`isset=false`), not a string cast.
///
/// Stateless, allocation-free, read-only and locale-independent. Non-string
/// diagnostics and other server context are unmodeled. This is a request hint
/// that ignores q values, not XML parsing, negotiation or authorization; JSON
/// and XML hints can both be true. No product HTTP behavior is changed.
pub fn wp_is_xml_request_bytes(accept: Option<&[u8]>, content_type: Option<&[u8]>) -> bool {
    accept.is_some_and(|header| {
        XML_TYPES
            .iter()
            .any(|kind| header.windows(kind.len()).any(|part| part == *kind))
    }) || content_type.is_some_and(|header| XML_TYPES.contains(&header))
}

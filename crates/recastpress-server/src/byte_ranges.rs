//! Finite single-byte-range profile for an identity, sealed representation.
//! Unsupported syntax and conditional requests retain the existing full reply.
//! This is not general conditional HTTP or multipart range handling.

use axum::http::{HeaderMap, Method, header};

const MAX_RANGE_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Plan {
    Full,
    Partial { start: usize, end: usize },
    Unsatisfiable,
}

pub(crate) fn select(
    headers: &HeaderMap,
    method: &Method,
    status: u16,
    length: usize,
    evaluated_conditions: bool,
) -> Plan {
    if *method != Method::GET || status != 200 || length == 0 {
        return Plan::Full;
    }
    // A private caller decision is needed before using conditional ranges.
    // Raw headers remain intact so the cache still rejects those requests.
    if !evaluated_conditions
        && [
            header::IF_RANGE,
            header::IF_MATCH,
            header::IF_NONE_MATCH,
            header::IF_MODIFIED_SINCE,
            header::IF_UNMODIFIED_SINCE,
        ]
        .into_iter()
        .any(|name| headers.contains_key(name))
    {
        return Plan::Full;
    }
    let all = headers.get_all(header::RANGE);
    let mut values = all.iter();
    let Some(value) = values.next() else {
        return Plan::Full;
    };
    if values.next().is_some() || value.as_bytes().len() > MAX_RANGE_BYTES {
        return Plan::Full;
    }
    value
        .to_str()
        .map_or(Plan::Full, |value| parse(value, length))
}

fn digits(value: &str) -> Option<&str> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let significant = value.trim_start_matches('0');
    Some(if significant.is_empty() {
        "0"
    } else {
        significant
    })
}

fn number(value: &str) -> usize {
    value.bytes().fold(0usize, |sum, byte| {
        sum.saturating_mul(10)
            .saturating_add(usize::from(byte - b'0'))
    })
}

fn parse(value: &str, length: usize) -> Plan {
    // RFC9110 permits ignoring Range for an empty representation. In particular
    // a positive suffix is not incorrectly classified as unsatisfiable here.
    if length == 0 {
        return Plan::Full;
    }
    let Some((unit, value)) = value.trim_matches([' ', '\t']).split_once('=') else {
        return Plan::Full;
    };
    if !unit.eq_ignore_ascii_case("bytes") {
        return Plan::Full;
    }
    let Some((first, last)) = value.split_once('-') else {
        return Plan::Full;
    };
    if first.is_empty() {
        let Some(suffix) = digits(last) else {
            return Plan::Full;
        };
        let suffix = number(suffix);
        return if suffix == 0 {
            Plan::Unsatisfiable
        } else {
            Plan::Partial {
                start: length.saturating_sub(suffix),
                end: length,
            }
        };
    }
    let Some(first) = digits(first) else {
        return Plan::Full;
    };
    let last = if last.is_empty() {
        None
    } else {
        let Some(last) = digits(last) else {
            return Plan::Full;
        };
        // Compare complete decimal values BEFORE saturating. Otherwise two
        // overflowing numbers can conceal an invalid descending range.
        if (first.len(), first) > (last.len(), last) {
            return Plan::Full;
        }
        Some(last)
    };
    let start = number(first);
    if start >= length {
        return Plan::Unsatisfiable;
    }
    let end = last.map_or(length, |last| number(last).saturating_add(1).min(length));
    Plan::Partial { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn plan(value: &str, length: usize) -> Plan {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_str(value).unwrap());
        select(&headers, &Method::GET, 200, length, false)
    }

    #[test]
    fn closed_open_and_suffix_ranges_follow_byte_positions_and_clamping() {
        for (wire, expected) in [
            ("bytes=2-4", Plan::Partial { start: 2, end: 5 }),
            ("bytes=2-", Plan::Partial { start: 2, end: 10 }),
            ("bytes=-3", Plan::Partial { start: 7, end: 10 }),
            ("BYTES=0002-00099", Plan::Partial { start: 2, end: 10 }),
            (" \tBytes=0-9\t ", Plan::Partial { start: 0, end: 10 }),
            ("bytes=-0", Plan::Unsatisfiable),
            ("bytes=10-", Plan::Unsatisfiable),
            ("bytes=4-2", Plan::Full),
        ] {
            assert_eq!(plan(wire, 10), expected, "{wire}");
        }
        for length in 1..=16 {
            for first in 0..=20 {
                for last in 0..=20 {
                    let expected = if first > last {
                        Plan::Full
                    } else if first >= length {
                        Plan::Unsatisfiable
                    } else {
                        Plan::Partial {
                            start: first,
                            end: (last + 1).min(length),
                        }
                    };
                    assert_eq!(plan(&format!("bytes={first}-{last}"), length), expected);
                }
            }
        }
    }

    #[test]
    fn huge_decimals_preserve_order_before_saturation_and_respect_wire_bound() {
        let smaller = "9".repeat(80);
        let larger = "1".repeat(81);
        assert_eq!(plan(&format!("bytes={larger}-{smaller}"), 10), Plan::Full);
        assert_eq!(
            plan(&format!("bytes={smaller}-{larger}"), 10),
            Plan::Unsatisfiable
        );
        assert_eq!(
            plan(&format!("bytes=1-{}", "9".repeat(200)), 10),
            Plan::Partial { start: 1, end: 10 }
        );
        assert_eq!(
            plan(&format!("bytes=-{}", "9".repeat(200)), 10),
            Plan::Partial { start: 0, end: 10 }
        );
        assert_eq!(
            plan(&format!("bytes=-{}1", "0".repeat(200)), 10),
            Plan::Partial { start: 9, end: 10 }
        );
        let boundary = format!("bytes={}0-1", "0".repeat(247));
        assert_eq!(boundary.len(), MAX_RANGE_BYTES);
        assert_eq!(plan(&boundary, 10), Plan::Partial { start: 0, end: 2 });
        assert_eq!(
            plan(&boundary.replacen("bytes=", "bytes=0", 1), 10),
            Plan::Full
        );
    }

    #[test]
    fn malformed_unknown_multiple_and_non_ascii_fields_keep_the_full_response() {
        for wire in [
            "",
            "bytes",
            "items=0-1",
            "bytes=",
            "bytes=-",
            "bytes=+1-2",
            "bytes=1--2",
            "bytes=1-+2",
            "bytes= 1-2",
            "bytes =1-2",
            "bytes=1 -2",
            "bytes=1- 2",
            "bytes=1-2,3-4",
            "bytes=1-2=3",
            "bytes=1.0-2",
            "bytes=1e0-2",
        ] {
            assert_eq!(plan(wire, 10), Plan::Full, "{wire:?}");
        }
        let mut headers = HeaderMap::new();
        headers.append(header::RANGE, HeaderValue::from_static("bytes=0-1"));
        headers.append(header::RANGE, HeaderValue::from_static("bytes=2-3"));
        assert_eq!(select(&headers, &Method::GET, 200, 10, false), Plan::Full);
        headers.insert(
            header::RANGE,
            HeaderValue::from_bytes(b"bytes=0-\xff").unwrap(),
        );
        assert_eq!(select(&headers, &Method::GET, 200, 10, false), Plan::Full);
    }

    #[test]
    fn empty_head_non_success_and_any_conditional_field_do_not_select_ranges() {
        for wire in ["bytes=0-0", "bytes=0-", "bytes=-1", "bytes=-0"] {
            assert_eq!(plan(wire, 0), Plan::Full);
        }
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=1-2"));
        for method in [Method::HEAD, Method::POST, Method::PUT] {
            assert_eq!(select(&headers, &method, 200, 10, false), Plan::Full);
        }
        for status in [201, 301, 302, 303, 307, 308, 404, 410] {
            assert_eq!(
                select(&headers, &Method::GET, status, 10, false),
                Plan::Full
            );
        }
        for name in [
            header::IF_RANGE,
            header::IF_MATCH,
            header::IF_NONE_MATCH,
            header::IF_MODIFIED_SINCE,
            header::IF_UNMODIFIED_SINCE,
        ] {
            let mut conditional = headers.clone();
            conditional.insert(name, HeaderValue::from_static(""));
            assert_eq!(
                select(&conditional, &Method::GET, 200, 10, false),
                Plan::Full
            );
        }
    }
}

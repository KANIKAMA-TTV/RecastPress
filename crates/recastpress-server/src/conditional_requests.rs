//! Bounded entity-tag conditions for the sealed GET/HEAD 200 representation.
//! No date/Last-Modified validator, mutation precondition, live publication or
//! general conditional HTTP contract is introduced. Header bounds and malformed
//! tag-list rejection are explicit prototype host policy, not product settings.

use axum::http::{HeaderMap, HeaderName, Method, header};

const MAX_FIELD_BYTES: usize = 4096;
const MAX_MEMBERS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Decision {
    Proceed { allow_range: bool },
    NotModified,
    Failed,
    BadRequest,
    HeaderTooLarge,
}

// Combining repeated list fields preserves quoted commas even across field
// boundaries. This bounded stack scratch is not part of shared-pool accounting.
struct Combined {
    bytes: [u8; MAX_FIELD_BYTES],
    len: usize,
    fields: usize,
}

fn combined(headers: &HeaderMap, name: &HeaderName) -> Result<Option<Combined>, Decision> {
    let mut result = Combined {
        bytes: [0; MAX_FIELD_BYTES],
        len: 0,
        fields: 0,
    };
    for value in headers.get_all(name) {
        let separator = usize::from(result.fields != 0);
        let len = result
            .len
            .checked_add(separator)
            .and_then(|len| len.checked_add(value.as_bytes().len()))
            .filter(|len| *len <= MAX_FIELD_BYTES)
            .ok_or(Decision::HeaderTooLarge)?;
        if separator != 0 {
            result.bytes[result.len] = b',';
            result.len += 1;
        }
        result.bytes[result.len..len].copy_from_slice(value.as_bytes());
        result.len = len;
        result.fields += 1;
    }
    Ok((result.fields != 0).then_some(result))
}

fn trim_ows(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(start, |offset| offset + 1);
    &bytes[start..end]
}

fn entity_tag(bytes: &[u8], mut offset: usize) -> Option<(bool, &[u8], usize)> {
    let weak = bytes.get(offset..offset + 2) == Some(b"W/");
    if weak {
        offset += 2;
    }
    if bytes.get(offset) != Some(&b'"') {
        return None;
    }
    offset += 1;
    let start = offset;
    while let Some(&byte) = bytes.get(offset) {
        if byte == b'"' {
            return Some((weak, &bytes[start..offset], offset + 1));
        }
        // etagc, not quoted-string: a backslash is literal, never an escape.
        if byte != 0x21 && !(0x23..=0x7e).contains(&byte) && byte < 0x80 {
            return None;
        }
        offset += 1;
    }
    None
}

fn list_matches(value: &Combined, current: &[u8], strong: bool) -> Result<bool, Decision> {
    let bytes = trim_ows(&value.bytes[..value.len]);
    if bytes == b"*" {
        return Ok(true);
    }
    let mut offset = 0;
    let mut members = 0;
    let mut matched = false;
    loop {
        members += 1;
        if members > MAX_MEMBERS {
            return Err(Decision::HeaderTooLarge);
        }
        while matches!(bytes.get(offset), Some(b' ' | b'\t')) {
            offset += 1;
        }
        if offset == bytes.len() {
            return Ok(matched);
        }
        if bytes[offset] == b',' {
            offset += 1;
            continue;
        }
        let (weak, opaque, end) = entity_tag(bytes, offset).ok_or(Decision::BadRequest)?;
        matched |= opaque == current && (!strong || !weak);
        offset = end;
        while matches!(bytes.get(offset), Some(b' ' | b'\t')) {
            offset += 1;
        }
        if offset == bytes.len() {
            return Ok(matched);
        }
        if bytes[offset] != b',' {
            return Err(Decision::BadRequest);
        }
        offset += 1;
        // A prior match is provisional until every remaining list member has
        // been parsed. Wildcard mixed with even an empty list member is invalid.
    }
}

pub(crate) fn evaluate(headers: &HeaderMap, method: &Method, current_opaque: &[u8]) -> Decision {
    if *method != Method::GET && *method != Method::HEAD {
        return Decision::Proceed { allow_range: false };
    }
    for (name, strong) in [(&header::IF_MATCH, true), (&header::IF_NONE_MATCH, false)] {
        let value = match combined(headers, name) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if let Some(value) = value {
            let matched = match list_matches(&value, current_opaque, strong) {
                Ok(matched) => matched,
                Err(error) => return error,
            };
            if strong && !matched {
                return Decision::Failed;
            }
            if !strong && matched {
                return Decision::NotModified;
            }
        }
    }
    if *method == Method::HEAD {
        return Decision::Proceed { allow_range: false };
    }
    // No modification date exists. Dates are not compared. This first profile
    // still conservatively suppresses Range when either date field is present.
    let date = headers.contains_key(header::IF_MODIFIED_SINCE)
        || headers.contains_key(header::IF_UNMODIFIED_SINCE);
    if !headers.contains_key(header::RANGE) {
        return Decision::Proceed { allow_range: !date };
    }
    let value = match combined(headers, &header::IF_RANGE) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let allow = match value {
        None => true,
        Some(value) => {
            let bytes = trim_ows(&value.bytes[..value.len]);
            value.fields == 1
                && entity_tag(bytes, 0).is_some_and(|(weak, opaque, end)| {
                    !weak && end == bytes.len() && opaque == current_opaque
                })
        }
    };
    Decision::Proceed {
        allow_range: allow && !date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(fields: &[(&str, &[u8])]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in fields {
            headers.append(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_bytes(value).unwrap(),
            );
        }
        headers
    }
    fn get(fields: &[(&str, &[u8])]) -> Decision {
        evaluate(&headers(fields), &Method::GET, b"current")
    }
    const ALLOW: Decision = Decision::Proceed { allow_range: true };
    const FULL: Decision = Decision::Proceed { allow_range: false };

    #[test]
    fn strong_match_precedes_weak_none_match_and_later_fields_do_not_override_a_final_decision() {
        assert_eq!(get(&[]), ALLOW);
        for value in [b"\"current\"".as_slice(), b"*", b"\"other\", \"current\""] {
            assert_eq!(get(&[("if-match", value)]), ALLOW);
        }
        for value in [b"W/\"current\"".as_slice(), b"\"other\"", b""] {
            assert_eq!(get(&[("if-match", value)]), Decision::Failed);
        }
        for value in [
            b"W/\"current\"".as_slice(),
            b"\"current\"",
            b"*",
            b"\"other\",W/\"current\"",
        ] {
            assert_eq!(get(&[("if-none-match", value)]), Decision::NotModified);
        }
        assert_eq!(get(&[("if-none-match", b"\"other\"")]), ALLOW);
        assert_eq!(
            get(&[
                ("if-match", b"\"other\""),
                ("if-none-match", b"\"current\"")
            ]),
            Decision::Failed
        );
        assert_eq!(
            get(&[("if-match", b"\"other\""), ("if-none-match", b"malformed")]),
            Decision::Failed
        );
        let huge = vec![b'x'; MAX_FIELD_BYTES + 1];
        assert_eq!(
            get(&[
                ("if-none-match", b"\"current\""),
                ("range", b"bytes=0-1"),
                ("if-range", &huge)
            ]),
            Decision::NotModified
        );
        let mut state = headers(&[
            ("if-match", b"\"current\""),
            ("if-none-match", b"W/\"current\""),
        ]);
        let before = state.clone();
        assert_eq!(
            evaluate(&state, &Method::HEAD, b"current"),
            Decision::NotModified
        );
        assert_eq!(state, before);
        state.insert(header::IF_MATCH, HeaderValue::from_static("\"other\""));
        assert_eq!(
            evaluate(&state, &Method::HEAD, b"current"),
            Decision::Failed
        );
    }

    #[test]
    fn complete_lists_handle_quoted_commas_raw_octets_literal_backslashes_and_repeated_fields() {
        let state = headers(&[("if-match", b"\"miss\""), ("if-match", b"\"a,b\\\xff\"")]);
        assert_eq!(evaluate(&state, &Method::GET, b"a,b\\\xff"), ALLOW);
        let state = headers(&[
            ("if-none-match", b"\"miss\""),
            ("if-none-match", b"W/\"a,b\\\xff\""),
        ]);
        assert_eq!(
            evaluate(&state, &Method::GET, b"a,b\\\xff"),
            Decision::NotModified
        );
        // Repeated fields are combined with a comma, including inside quotes.
        let state = headers(&[("if-match", b"\"a"), ("if-match", b"b\"")]);
        assert_eq!(evaluate(&state, &Method::GET, b"a,b"), ALLOW);
        for value in [
            b"".as_slice(),
            b",",
            b" , \t, ",
            b"\"other\",,",
            b", \"other\" ,",
        ] {
            assert_eq!(get(&[("if-none-match", value)]), ALLOW);
            assert_eq!(get(&[("if-match", value)]), Decision::Failed);
        }
        assert_eq!(
            evaluate(&headers(&[("if-match", b"\"\"")]), &Method::GET, b""),
            ALLOW
        );
        for value in [
            b"w/\"current\"".as_slice(),
            b"W/ \"current\"",
            b"\"current\",bad",
            b"\"current\"junk",
            b"\"current\",*",
            b"*,",
            b",*",
            b"*,*",
            b"\"a b\"",
            b"\"a\tb\"",
            b"\"unterminated",
            b"\"a\\\"b\"",
        ] {
            assert_eq!(
                get(&[("if-match", value)]),
                Decision::BadRequest,
                "{value:?}"
            );
            assert_eq!(
                get(&[("if-none-match", value)]),
                Decision::BadRequest,
                "{value:?}"
            );
        }
        // Locked HeaderValue rejects DEL/C0 before this evaluator can receive
        // them; exercise the shared raw parser directly for those bytes.
        for bytes in [b"\"a\x7fb\"".as_slice(), b"\"a\0b\""] {
            let mut raw = Combined {
                bytes: [0; MAX_FIELD_BYTES],
                len: bytes.len(),
                fields: 1,
            };
            raw.bytes[..bytes.len()].copy_from_slice(bytes);
            assert_eq!(
                list_matches(&raw, b"current", true),
                Err(Decision::BadRequest)
            );
        }
        assert_eq!(
            get(&[("if-match", b"*"), ("if-match", b"")]),
            Decision::BadRequest
        );
        assert_eq!(
            get(&[("if-none-match", b"*"), ("if-none-match", b"\"current\"")]),
            Decision::BadRequest
        );
    }

    #[test]
    fn aggregate_bytes_and_member_counts_are_bounded_including_combining_commas_and_empty_members()
    {
        let tag = format!("\"{}\"", "x".repeat(MAX_FIELD_BYTES - 2));
        assert_eq!(tag.len(), MAX_FIELD_BYTES);
        assert_eq!(get(&[("if-none-match", tag.as_bytes())]), ALLOW);
        let long = format!("{tag} ");
        assert_eq!(
            get(&[("if-none-match", long.as_bytes())]),
            Decision::HeaderTooLarge
        );
        let left = format!("\"{}\"", "x".repeat(2045));
        let right = format!("\"{}\"", "y".repeat(2046));
        assert_eq!(left.len() + right.len() + 1, MAX_FIELD_BYTES);
        assert_eq!(
            get(&[
                ("if-match", left.as_bytes()),
                ("if-match", right.as_bytes())
            ]),
            Decision::Failed
        );
        let right = format!("{right} ");
        assert_eq!(
            get(&[
                ("if-match", left.as_bytes()),
                ("if-match", right.as_bytes())
            ]),
            Decision::HeaderTooLarge
        );
        for members in [MAX_MEMBERS, MAX_MEMBERS + 1] {
            let empty = ",".repeat(members - 1);
            let tags = vec!["\"other\""; members].join(",");
            let expected = if members == MAX_MEMBERS {
                ALLOW
            } else {
                Decision::HeaderTooLarge
            };
            assert_eq!(get(&[("if-none-match", empty.as_bytes())]), expected);
            assert_eq!(get(&[("if-none-match", tags.as_bytes())]), expected);
        }
        let late = format!("\"current\",{}", vec!["\"other\""; MAX_MEMBERS].join(","));
        assert_eq!(
            get(&[("if-none-match", late.as_bytes())]),
            Decision::HeaderTooLarge
        );
    }

    #[test]
    fn if_range_only_allows_an_exact_single_strong_tag_and_never_returns_failed() {
        let base = ("range", b"bytes=0-1".as_slice());
        assert_eq!(get(&[base]), ALLOW);
        assert_eq!(get(&[base, ("if-range", b" \t\"current\"\t ")]), ALLOW);
        for value in [
            b"".as_slice(),
            b"W/\"current\"",
            b"\"other\"",
            b"*",
            b"\"current\",\"current\"",
            b"Sat, 29 Oct 1994 19:43:31 GMT",
            b"bad",
            b"\"current\"junk",
        ] {
            assert_eq!(get(&[base, ("if-range", value)]), FULL);
        }
        assert_eq!(
            get(&[
                base,
                ("if-range", b"\"current\""),
                ("if-range", b"\"current\"")
            ]),
            FULL
        );
        let padded = format!("\"current\"{}", " ".repeat(MAX_FIELD_BYTES - 9));
        assert_eq!(padded.len(), MAX_FIELD_BYTES);
        assert_eq!(get(&[base, ("if-range", padded.as_bytes())]), ALLOW);
        let over = format!("{padded} ");
        assert_eq!(
            get(&[base, ("if-range", over.as_bytes())]),
            Decision::HeaderTooLarge
        );
        let many = vec![b'x'; MAX_FIELD_BYTES];
        assert_eq!(
            get(&[base, ("if-range", &many), ("if-range", b"")]),
            Decision::HeaderTooLarge
        );
    }

    #[test]
    fn dates_only_suppress_range_after_tag_preconditions_and_headers_are_not_rewritten() {
        for date in ["if-modified-since", "if-unmodified-since"] {
            for value in [
                b"".as_slice(),
                b"bad date",
                b"Sat, 29 Oct 1994 19:43:31 GMT",
            ] {
                let state = headers(&[
                    ("range", b"bytes=0-1"),
                    ("if-range", b"\"current\""),
                    (date, value),
                ]);
                let old = state.clone();
                assert_eq!(evaluate(&state, &Method::GET, b"current"), FULL);
                assert_eq!(state, old);
                assert_eq!(
                    get(&[("if-match", b"\"other\""), (date, value)]),
                    Decision::Failed
                );
                assert_eq!(
                    get(&[("if-none-match", b"W/\"current\""), (date, value)]),
                    Decision::NotModified
                );
            }
        }
    }

    #[test]
    fn head_or_absent_range_ignores_if_range_even_when_huge_and_other_methods_skip_all_conditions()
    {
        let huge = vec![b'x'; MAX_FIELD_BYTES + 1];
        assert_eq!(get(&[("if-range", &huge)]), ALLOW);
        let state = headers(&[("range", b"bytes=0-1"), ("if-range", &huge)]);
        assert_eq!(evaluate(&state, &Method::HEAD, b"current"), FULL);
        let state = headers(&[
            ("if-match", &huge),
            ("if-none-match", b"*"),
            ("if-range", &huge),
        ]);
        for method in [Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS] {
            assert_eq!(evaluate(&state, &method, b"current"), FULL);
        }
    }
}

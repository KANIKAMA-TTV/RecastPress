//! Parse a complete, successfully executed CGI response for the P0 fixture.
//!
//! The caller must first validate execution, output budgets, and request cleanup.
//! This parser is neither an HTTP request parser nor proof of product readiness.
//! CGI framing follows RFC 3875 section 6. HTTP body/length rules follow RFC 9110
//! sections 8.6 and 15.3.5/15.4.5. HEAD suppression belongs to the HTTP adapter,
//! which knows the request method; parse the full generated representation here.
use anyhow::{Context, Result, ensure};

pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_HEADER_FIELDS: usize = 64;

#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// Lowercase field names, original value bytes, original order and duplicates.
    /// CGI Status and Content-Length are excluded. The transport determines its
    /// framing from the completed body, with method/status-specific suppression.
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
    /// A 304 Content-Length describes the selected 200 representation, not this
    /// response's empty body. It is retained separately and MUST NOT be sent until
    /// the HTTP layer verifies it against the selected representation. Omitting
    /// it is valid. All other statuses leave this field None.
    pub representation_length: Option<u64>,
}

pub fn parse(output: &[u8]) -> Result<Response> {
    let mut position = 0;
    let mut crlf_style = None;
    let mut count = 0;
    let mut headers = Vec::new();
    let mut status = None;
    let mut length = None;
    let mut location = false;
    let mut content_type = false;
    loop {
        let end = output.len().min(MAX_HEADER_BYTES);
        ensure!(
            position < end,
            "missing CGI header terminator or header limit"
        );
        let newline = output[position..end]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| position + offset)
            .context("missing CGI header terminator or header limit")?;
        let raw_line = &output[position..newline];
        let is_crlf = raw_line.last() == Some(&b'\r');
        if let Some(previous) = crlf_style {
            ensure!(previous == is_crlf, "mixed CGI header line endings");
        }
        crlf_style = Some(is_crlf);
        let line = if is_crlf {
            &raw_line[..raw_line.len() - 1]
        } else {
            raw_line
        };
        position = newline + 1;
        if line.is_empty() {
            break;
        }
        count += 1;
        ensure!(count <= MAX_HEADER_FIELDS, "too many CGI header fields");
        ensure!(!matches!(line[0], b' ' | b'\t'), "folded CGI header");
        let colon = line
            .iter()
            .position(|byte| *byte == b':')
            .context("CGI header without colon")?;
        let name = &line[..colon];
        ensure!(
            !name.is_empty() && name.iter().all(|byte| token(*byte)),
            "invalid CGI header name"
        );
        let name = std::str::from_utf8(name)?.to_ascii_lowercase();
        let value = trim_ows(&line[colon + 1..]);
        ensure!(
            value
                .iter()
                .all(|byte| *byte == b'\t' || (*byte >= b' ' && *byte != 0x7f)),
            "control byte in CGI header value"
        );
        ensure!(
            !matches!(
                name.as_str(),
                "connection"
                    | "keep-alive"
                    | "proxy-connection"
                    | "te"
                    | "trailer"
                    | "transfer-encoding"
                    | "upgrade"
            ),
            "hop-by-hop CGI header"
        );
        match name.as_str() {
            "status" => {
                ensure!(status.is_none(), "duplicate CGI Status");
                ensure!(
                    value.len() >= 3 && value[..3].iter().all(u8::is_ascii_digit),
                    "invalid CGI Status"
                );
                ensure!(
                    value.len() == 3 || value[3] == b' ',
                    "invalid CGI Status separator"
                );
                let code = (value[0] - b'0') as u16 * 100
                    + (value[1] - b'0') as u16 * 10
                    + (value[2] - b'0') as u16;
                ensure!(
                    (200..=599).contains(&code),
                    "CGI Status is not a supported final status"
                );
                status = Some(code);
            }
            "content-length" => {
                ensure!(length.is_none(), "duplicate CGI Content-Length");
                ensure!(
                    !value.is_empty() && value.iter().all(u8::is_ascii_digit),
                    "invalid CGI Content-Length"
                );
                length = Some(
                    std::str::from_utf8(value)?
                        .parse::<u64>()
                        .context("CGI Content-Length overflow")?,
                );
            }
            "location" => {
                ensure!(!location, "duplicate CGI Location");
                location = true;
                headers.push((name, value.to_vec()));
            }
            "content-type" => {
                ensure!(!content_type, "duplicate CGI Content-Type");
                content_type = true;
                headers.push((name, value.to_vec()));
            }
            _ => headers.push((name, value.to_vec())),
        }
    }
    let status = status.unwrap_or(if location { 302 } else { 200 });
    let body = &output[position..];
    let representation_length = match status {
        204 => {
            ensure!(body.is_empty(), "204 CGI response has content");
            ensure!(length.is_none(), "204 CGI response has Content-Length");
            None
        }
        304 => {
            ensure!(body.is_empty(), "304 CGI response has content");
            length
        }
        _ => {
            if status == 205 {
                ensure!(body.is_empty(), "205 CGI response has content");
            }
            if let Some(length) = length {
                ensure!(
                    length == u64::try_from(body.len())?,
                    "CGI Content-Length does not match completed body"
                );
            }
            None
        }
    };
    Ok(Response {
        status,
        headers,
        body: body.to_vec(),
        representation_length,
    })
}

fn token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while matches!(value.first(), Some(b' ' | b'\t')) {
        value = &value[1..];
    }
    while matches!(value.last(), Some(b' ' | b'\t')) {
        value = &value[..value.len() - 1];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordpress_error_preserves_separate_cookies_and_binary_body() {
        let output = b"Status: 404 Not Found\r\nContent-Type: application/octet-stream\r\nSet-Cookie: wp_a=1; HttpOnly\r\nSet-Cookie: wp_b=2; SameSite=Lax\r\nContent-Length: 5\r\n\r\n\0\xffa\r\n";
        let response = parse(output).unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.body, b"\0\xffa\r\n");
        assert_eq!(
            response.headers,
            vec![
                ("content-type".into(), b"application/octet-stream".to_vec()),
                ("set-cookie".into(), b"wp_a=1; HttpOnly".to_vec()),
                ("set-cookie".into(), b"wp_b=2; SameSite=Lax".to_vec()),
            ]
        );
        assert_eq!(response.representation_length, None);
    }

    #[test]
    fn default_status_redirect_and_explicit_redirect_are_distinct() {
        assert_eq!(
            parse(b"Content-type: text/html\n\npage").unwrap().status,
            200
        );
        let implicit = parse(b"Location: /next\r\n\r\n").unwrap();
        assert_eq!(implicit.status, 302);
        assert_eq!(
            implicit.headers,
            vec![("location".into(), b"/next".to_vec())]
        );
        assert_eq!(
            parse(b"Status: 307 Temporary Redirect\r\nLocation: /next\r\n\r\n")
                .unwrap()
                .status,
            307
        );
        assert_eq!(
            parse(b"Status: 200\nLocation: /canonical\n\npage")
                .unwrap()
                .status,
            200
        );
    }

    #[test]
    fn rejects_ambiguous_status_and_nonfinal_status() {
        let invalid: &[&[u8]] = &[
            b"Status: 200 OK\nSTATUS: 404 Not Found\n\n",
            b"Status: 101 Switching Protocols\n\n",
            b"Status: 199 Interim\n\n",
            b"Status: 600 Invalid\n\n",
            b"Status: 200OK\n\n",
            b"Status: +200 OK\n\n",
            b"Status: 20 OK\n\n",
        ];
        for output in invalid {
            assert!(parse(output).is_err(), "{output:?}");
        }
    }

    #[test]
    fn rejects_smuggling_length_forms_even_with_identical_duplicates() {
        let invalid: &[&[u8]] = &[
            b"Content-Length: 3\ncontent-length: 3\n\nabc",
            b"Content-Length: 3\nContent-Length: 4\n\nabc",
            b"Content-Length: 3, 3\n\nabc",
            b"Content-Length: +3\n\nabc",
            b"Content-Length: 3 0\n\nabc",
            b"Content-Length: 18446744073709551616\n\nabc",
            b"Content-Length: 2\n\nabc",
            b"Content-Length: 4\n\nabc",
            b"Content-Length:\n\n",
        ];
        for output in invalid {
            assert!(parse(output).is_err(), "{output:?}");
        }
        assert_eq!(parse(b"Content-Length: 003\n\nabc").unwrap().body, b"abc");
    }

    #[test]
    fn rejects_folding_controls_mixed_endings_and_hop_headers() {
        let invalid: &[&[u8]] = &[
            b"Content-Type: text/plain\n X: folded\n\nbody",
            b"Bad Name: x\n\n",
            b"X: a\rb\n\n",
            b"X: a\0b\n\n",
            b"X: a\x7fb\n\n",
            b"X: a\r\nY: b\n\n",
            b"X: a\n\r\n",
            b"Connection: X-Private\nX-Private: secret\n\n",
            b"Transfer-Encoding: chunked\n\n0\r\n\r\n",
            b"Keep-Alive: timeout=10\n\n",
            b"Trailer: Digest\n\n",
            b"Content-Type: text/plain\n",
        ];
        for output in invalid {
            assert!(parse(output).is_err(), "{output:?}");
        }
    }

    #[test]
    fn preserves_header_value_bytes_but_never_reparses_body() {
        let response =
            parse(b"X-Note: \t\xff\t ok \t\n\nStatus: 500\r\n\r\nRP_P0 CACHE_SET 7\n\0\xff")
                .unwrap();
        assert_eq!(
            response.headers,
            vec![("x-note".into(), b"\xff\t ok".to_vec())]
        );
        assert_eq!(response.status, 200);
        assert_eq!(
            response.body,
            b"Status: 500\r\n\r\nRP_P0 CACHE_SET 7\n\0\xff"
        );
    }

    #[test]
    fn header_budgets_include_the_terminating_blank_line() {
        let mut exact = b"X: ".to_vec();
        exact.extend(vec![b'a'; MAX_HEADER_BYTES - 7]);
        exact.extend_from_slice(b"\r\n\r\n");
        assert_eq!(exact.len(), MAX_HEADER_BYTES);
        assert!(parse(&exact).is_ok());
        exact.insert(3, b'a');
        assert!(parse(&exact).is_err());
        let mut fields = Vec::new();
        for index in 0..MAX_HEADER_FIELDS {
            fields.extend_from_slice(format!("X-{index}: value\n").as_bytes());
        }
        fields.push(b'\n');
        assert!(parse(&fields).is_ok());
        fields.pop();
        fields.extend_from_slice(b"X-extra: value\n\n");
        assert!(parse(&fields).is_err());
    }

    #[test]
    fn no_content_and_not_modified_keep_their_different_length_rules() {
        let empty = parse(b"Status: 204 No Content\r\n\r\n").unwrap();
        assert!(empty.body.is_empty());
        assert!(parse(b"Status: 204 No Content\nContent-Length: 0\n\n").is_err());
        assert!(parse(b"Status: 204 No Content\n\nx").is_err());
        let cached =
            parse(b"Status: 304 Not Modified\nContent-Length: 12345\nETag: \"cached\"\n\n")
                .unwrap();
        assert!(cached.body.is_empty());
        assert_eq!(cached.representation_length, Some(12345));
        assert_eq!(
            cached.headers,
            vec![("etag".into(), b"\"cached\"".to_vec())]
        );
        assert!(parse(b"Status: 304 Not Modified\n\nx").is_err());
        assert!(parse(b"Status: 205 Reset Content\n\nx").is_err());
        assert!(parse(b"Status: 205 Reset Content\nContent-Length: 0\n\n").is_ok());
    }

    #[test]
    fn singleton_fields_cannot_be_combined_but_repeatable_fields_survive() {
        assert!(parse(b"Location: /a\nlocation: /b\n\n").is_err());
        assert!(parse(b"Content-Type: text/plain\nContent-Type: text/html\n\n").is_err());
        let response = parse(b"Vary: Accept\nVary: Cookie\n\n").unwrap();
        assert_eq!(
            response.headers,
            vec![
                ("vary".into(), b"Accept".to_vec()),
                ("vary".into(), b"Cookie".to_vec())
            ]
        );
    }
}

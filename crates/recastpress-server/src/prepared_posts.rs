//! Offline preparation of explicitly selected article text, not public selection.
//!
//! No raw editing row, status, password, author, slug, shortcode, block, theme,
//! executable module or file source enters this model. The caller supplies only
//! text already selected for anonymous publication and an exact target. The fixed
//! layout is a development sample, not a WordPress or Wasm theme implementation.
//! Literal text is escaped; it is neither interpreted as HTML nor sanitized for
//! private information. No DB, filesystem, network, listener or publish is used.

use crate::bundle;
use serde::{Deserialize, Serialize};
use std::{fmt, io, io::Write};

pub const MAX_POSTS_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const MIME: &str = "text/html; charset=utf-8";
const BEFORE_TITLE: &[u8] = b"<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>";
const BEFORE_HEADING: &[u8] = b"</title></head><body><main><article><h1>";
const BEFORE_BODY: &[u8] = b"</h1><div class=\"post-body\" style=\"white-space:pre-wrap\">";
const AFTER_BODY: &[u8] = b"</div></article></main></body></html>\n";
const SHELL_BYTES: usize =
    BEFORE_TITLE.len() + BEFORE_HEADING.len() + BEFORE_BODY.len() + AFTER_BODY.len();

/// Only caller-selected public text and its explicit request target.
/// No constructor from a raw editing record or automatic route policy exists.
#[derive(Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PreparedPost {
    pub target: String,
    pub title: String,
    pub body_text: String,
}

impl fmt::Debug for PreparedPost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedPost")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedPostsError {
    InputLimit,
    InvalidInput,
    InvalidText,
    OutputLimit,
    Admission,
}

impl fmt::Display for PreparedPostsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputLimit => "prepared posts input exceeds its byte bound",
            Self::InvalidInput => "invalid prepared posts input",
            Self::InvalidText => "prepared post text contains NUL",
            Self::OutputLimit => "prepared posts output exceeds a byte or allocation bound",
            Self::Admission => "prepared posts bundle failed admission",
        })
    }
}

impl std::error::Error for PreparedPostsError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PostsInput {
    format_version: u32,
    posts: Vec<PreparedPost>,
}

/// Strict experimental input schema 1. All fields are required; duplicate and
/// unknown fields are rejected. No input value is retained in an error message.
pub fn parse_input(input: &[u8]) -> Result<Vec<PreparedPost>, PreparedPostsError> {
    if input.len() as u64 > MAX_POSTS_INPUT_BYTES {
        return Err(PreparedPostsError::InputLimit);
    }
    let parsed: PostsInput =
        serde_json::from_slice(input).map_err(|_| PreparedPostsError::InvalidInput)?;
    if parsed.format_version != 1 {
        return Err(PreparedPostsError::InvalidInput);
    }
    Ok(parsed.posts)
}

struct CappedWriter<'a> {
    bytes: Option<&'a mut Vec<u8>>,
    length: usize,
    limit: usize,
}

impl Write for CappedWriter<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let next = self
            .length
            .checked_add(input.len())
            .filter(|length| *length <= self.limit)
            .ok_or_else(|| io::Error::other("prepared output limit"))?;
        if let Some(bytes) = &mut self.bytes {
            bytes.extend_from_slice(input);
        }
        self.length = next;
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn text(writer: &mut CappedWriter<'_>, input: &str) -> io::Result<()> {
    // Operate on UTF-8 byte runs. Replacement only occurs for ASCII bytes, so
    // unchanged multi-byte sequences remain exact. CR references avoid HTML's
    // CR/LF input normalization. A div avoids pre's initial-LF removal rule.
    let mut start = 0;
    for (offset, byte) in input.bytes().enumerate() {
        let replacement: &[u8] = match byte {
            b'&' => b"&amp;",
            b'<' => b"&lt;",
            b'>' => b"&gt;",
            b'\"' => b"&quot;",
            b'\'' => b"&#39;",
            b'\r' => b"&#13;",
            _ => continue,
        };
        writer.write_all(&input.as_bytes()[start..offset])?;
        writer.write_all(replacement)?;
        start = offset + 1;
    }
    writer.write_all(&input.as_bytes()[start..])
}

fn render(writer: &mut CappedWriter<'_>, post: &PreparedPost) -> io::Result<()> {
    writer.write_all(BEFORE_TITLE)?;
    text(writer, &post.title)?;
    writer.write_all(BEFORE_HEADING)?;
    text(writer, &post.title)?;
    writer.write_all(BEFORE_BODY)?;
    text(writer, &post.body_text)?;
    writer.write_all(AFTER_BODY)
}

fn valid_text(post: &PreparedPost) -> Result<(), PreparedPostsError> {
    if post.title.contains('\0') || post.body_text.contains('\0') {
        return Err(PreparedPostsError::InvalidText);
    }
    Ok(())
}

/// Render the fixed sample layout with literal UTF-8 text. Empty title/body is
/// accepted; no WordPress content/status policy is inferred. NUL is rejected
/// because HTML parsing would replace it. CR, LF, tabs and other String data are
/// retained as text; no general HTML validity or privacy sanitation is claimed.
///
/// Measure and emit through one traversal, cap the complete escaped HTML, and
/// reserve fallibly before emission. The hard 8 MiB ceiling also applies when
/// the caller supplies a larger allowance. Inputs/capacity/temporary work are
/// unpooled and this final-length bound is not a process RAM limit.
pub fn render_post(
    post: &PreparedPost,
    max_body_bytes: usize,
) -> Result<Vec<u8>, PreparedPostsError> {
    valid_text(post)?;
    let mut counter = CappedWriter {
        bytes: None,
        length: 0,
        limit: max_body_bytes.min(bundle::MAX_BUNDLE_BYTES as usize),
    };
    render(&mut counter, post).map_err(|_| PreparedPostsError::OutputLimit)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(counter.length)
        .map_err(|_| PreparedPostsError::OutputLimit)?;
    let mut output = CappedWriter {
        bytes: Some(&mut bytes),
        length: 0,
        limit: counter.length,
    };
    render(&mut output, post).map_err(|_| PreparedPostsError::OutputLimit)?;
    if output.length != counter.length {
        return Err(PreparedPostsError::OutputLimit);
    }
    Ok(bytes)
}

#[derive(Serialize)]
struct ExportPage<'a> {
    target: &'a str,
    status: u16,
    content_type: &'static str,
    body: &'a str,
}

#[derive(Serialize)]
struct ExportBundle<'a> {
    format_version: u32,
    pages: Vec<ExportPage<'a>>,
}

/// Export the existing bundle schema 1, in caller order, with a terminal LF.
/// This never chooses public records, derives slugs, opens files, publishes or
/// interprets WordPress HTML. Every completed artifact is passed through the
/// actual bundle admission (including duplicate/reserved/invalid targets).
///
/// Each escaped HTML body and the complete serialized JSON have separate caps.
/// Both the hard 8 MiB bundle bound and caller's final-wire cap include the LF.
/// Rendering strings, page descriptors, input and admission's validation copies
/// are temporary unpooled work. Failure returns no partial output bytes.
pub fn export_bundle(
    posts: &[PreparedPost],
    max_body_bytes: usize,
    max_bundle_bytes: usize,
) -> Result<Vec<u8>, PreparedPostsError> {
    for post in posts {
        valid_text(post)?;
    }
    let limit = max_bundle_bytes.min(bundle::MAX_BUNDLE_BYTES as usize);
    // Every rendered body contains at least the fixed shell. This is a proven
    // lower bound, not an additional arbitrary record-count policy.
    if posts.len() > limit / SHELL_BYTES {
        return Err(PreparedPostsError::OutputLimit);
    }
    let mut bodies = Vec::new();
    bodies
        .try_reserve_exact(posts.len())
        .map_err(|_| PreparedPostsError::OutputLimit)?;
    let mut body_total = 0usize;
    for post in posts {
        let remaining = limit
            .checked_sub(body_total)
            .ok_or(PreparedPostsError::OutputLimit)?;
        let bytes = render_post(post, max_body_bytes.min(remaining))?;
        body_total = body_total
            .checked_add(bytes.len())
            .ok_or(PreparedPostsError::OutputLimit)?;
        bodies.push(String::from_utf8(bytes).map_err(|_| PreparedPostsError::InvalidText)?);
    }
    let mut pages = Vec::new();
    pages
        .try_reserve_exact(posts.len())
        .map_err(|_| PreparedPostsError::OutputLimit)?;
    for (post, body) in posts.iter().zip(&bodies) {
        pages.push(ExportPage {
            target: &post.target,
            status: 200,
            content_type: MIME,
            body,
        });
    }
    let exported = ExportBundle {
        format_version: 1,
        pages,
    };
    let mut counter = CappedWriter {
        bytes: None,
        length: 0,
        limit,
    };
    serde_json::to_writer(&mut counter, &exported).map_err(|_| PreparedPostsError::OutputLimit)?;
    counter
        .write_all(b"\n")
        .map_err(|_| PreparedPostsError::OutputLimit)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(counter.length)
        .map_err(|_| PreparedPostsError::OutputLimit)?;
    let mut output = CappedWriter {
        bytes: Some(&mut bytes),
        length: 0,
        limit: counter.length,
    };
    serde_json::to_writer(&mut output, &exported).map_err(|_| PreparedPostsError::OutputLimit)?;
    output
        .write_all(b"\n")
        .map_err(|_| PreparedPostsError::OutputLimit)?;
    if output.length != counter.length {
        return Err(PreparedPostsError::OutputLimit);
    }
    bundle::parse(&bytes).map_err(|_| PreparedPostsError::Admission)?;
    Ok(bytes)
}

pub fn prepare_bundle(
    input: &[u8],
    max_body_bytes: usize,
    max_bundle_bytes: usize,
) -> Result<Vec<u8>, PreparedPostsError> {
    export_bundle(&parse_input(input)?, max_body_bytes, max_bundle_bytes)
}

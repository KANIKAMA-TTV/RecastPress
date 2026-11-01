//! Fixed, public development fixture. No input, DB, authorization or HTTP host.
//! The filtered title is escaped in a separate step before fixed HTML assembly.
use recastpress_wp::{
    PreparedTitlePost, TexturizeLimits, esc_html_utf8_empty_filters_bytes,
    get_the_title_prepared_frontend_default_english_no_html_bytes,
};
use serde_json::json;
use std::io::{self, Write};

fn run() -> Result<(), ()> {
    if std::env::args_os().len() != 1 {
        return Err(());
    }
    let post = PreparedTitlePost::new(
        1,
        "   \"Wordpress & friends\" -- 9x9 &a; &lt;b&gt; 日本   ".as_bytes(),
        b"",
        b"publish",
    )
    .map_err(|_| ())?;
    let filtered = get_the_title_prepared_frontend_default_english_no_html_bytes(
        &post,
        TexturizeLimits::new(4096, 65536, 4096),
    )
    .map_err(|_| ())?;
    let escaped = esc_html_utf8_empty_filters_bytes(&filtered, 8192).map_err(|_| ())?;
    let escaped = std::str::from_utf8(&escaped).map_err(|_| ())?;
    let body = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>{escaped}</title></head><body><h1>{escaped}</h1><p>Fixed prepared title fixture.</p></body></html>\n"
    );
    let payload = serde_json::to_vec(&json!({
        "format_version": 1,
        "pages": [{"target": "/title-fixture", "status": 200,
            "content_type": "text/html; charset=utf-8", "body": body}]
    }))
    .map_err(|_| ())?;
    if payload.len() > 65536 {
        return Err(());
    }
    let mut output = io::stdout().lock();
    output.write_all(&payload).map_err(|_| ())?;
    output.write_all(b"\n").map_err(|_| ())?;
    output.flush().map_err(|_| ())
}

fn main() -> std::process::ExitCode {
    if run().is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        let _ = io::stderr().write_all(b"title fixture failed\n");
        std::process::ExitCode::from(2)
    }
}

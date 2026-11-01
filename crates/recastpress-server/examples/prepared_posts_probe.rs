//! Fixed caller-selected text fixture only; no raw editing row or publication.
use recastpress_server::prepared_posts::{PreparedPost, export_bundle, parse_input, render_post};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let posts = vec![
        PreparedPost {
            target: "/chosen?b=2&a=1".into(),
            title: "Title 日本 & < > \" '".into(),
            body_text: "\n<script>alert('literal')</script>\r\n\t&copy; 日本語 🦀".into(),
        },
        PreparedPost {
            target: "/empty".into(),
            title: "".into(),
            body_text: "".into(),
        },
    ];
    let bundle = export_bundle(&posts, 65536, 131072)?;
    let first_html = render_post(&posts[0], 65536)?;
    let empty_html = render_post(&posts[1], 65536)?;
    let parsed: serde_json::Value = serde_json::from_slice(&bundle)?;
    assert_eq!(
        parsed["pages"][0]["body"].as_str().unwrap().as_bytes(),
        first_html
    );
    assert_eq!(
        parsed["pages"][1]["body"].as_str().unwrap().as_bytes(),
        empty_html
    );
    let input = json!({"format_version":1,"posts":[
        {"target":posts[0].target,"title":posts[0].title,"body_text":posts[0].body_text},
        {"target":posts[1].target,"title":posts[1].title,"body_text":posts[1].body_text}
    ]});
    assert_eq!(parse_input(&serde_json::to_vec(&input)?)?, posts);
    println!(
        "{}",
        json!({"html":String::from_utf8(first_html)?,
        "empty_html":String::from_utf8(empty_html)?,"bundle":String::from_utf8(bundle)?,
        "input":input,"scope":"fixed_explicitly_prepared_text_only"})
    );
    Ok(())
}

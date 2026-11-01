use recastpress_server::prepared_posts::{
    MAX_POSTS_INPUT_BYTES, PreparedPost, PreparedPostsError, export_bundle, parse_input,
    prepare_bundle, render_post,
};
use serde_json::{Value, json};

fn post() -> PreparedPost {
    PreparedPost {
        target: "/chosen?b=2&a=1".into(),
        title: "Title 日本 & < > \" '".into(),
        body_text: "\n<script>alert('literal')</script>\r\n\t&copy; 日本語 🦀".into(),
    }
}

#[test]
fn strict_input_never_accepts_raw_editing_fields_or_missing_duplicate_null_fields() {
    let parsed =
        parse_input(br#"{"format_version":1,"posts":[{"target":"/","title":"","body_text":""}]}"#)
            .unwrap();
    assert_eq!(parsed[0].title, "");
    assert_eq!(parsed[0].body_text, "");
    for field in [
        "post_status",
        "post_password",
        "post_author",
        "post_name",
        "meta",
        "id",
    ] {
        let mut value =
            json!({"format_version":1,"posts":[{"target":"/","title":"safe","body_text":"safe"}]});
        value["posts"][0][field] = json!("SECRET_PRIVATE_VALUE");
        let error = parse_input(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert_eq!(error, PreparedPostsError::InvalidInput);
        assert!(!format!("{error:?} {error}").contains("SECRET_PRIVATE_VALUE"));
    }
    for wire in [
        br#"{"format_version":1,"posts":[],"private":"SECRET"}"#.as_slice(),
        br#"{"format_version":1,"format_version":1,"posts":[]}"#,
        br#"{"format_version":2,"posts":[]}"#,
        br#"{"format_version":1.0,"posts":[]}"#,
        br#"{"format_version":true,"posts":[]}"#,
        br#"{"format_version":1,"posts":[{"target":"/","title":"x"}]}"#,
        br#"{"format_version":1,"posts":[{"target":"/","title":null,"body_text":"x"}]}"#,
        br#"{"format_version":1,"posts":[{"target":"/","title":"x","body_text":"a","body_text":"b"}]}"#,
    ] {
        assert_eq!(parse_input(wire), Err(PreparedPostsError::InvalidInput));
    }
    assert!(!format!("{:?}", post()).contains("chosen"));
    assert!(!format!("{:?}", post()).contains("literal"));
}

#[test]
fn input_byte_bound_has_an_exact_inclusive_boundary_and_no_invalid_utf8_admission() {
    let mut input = br#"{"format_version":1,"posts":[]}"#.to_vec();
    input.resize(MAX_POSTS_INPUT_BYTES as usize, b' ');
    assert!(parse_input(&input).unwrap().is_empty());
    input.push(b' ');
    assert_eq!(parse_input(&input), Err(PreparedPostsError::InputLimit));
    assert_eq!(parse_input(b"\xff"), Err(PreparedPostsError::InvalidInput));
}

#[test]
fn literal_html_output_escapes_data_preserves_unicode_and_rejects_nul_without_input_mutation() {
    let post = post();
    let before = post.clone();
    let bytes = render_post(&post, 65536).unwrap();
    let html = String::from_utf8(bytes).unwrap();
    assert!(html.contains("Title 日本 &amp; &lt; &gt; &quot; &#39;"));
    assert!(html.contains(
        "&lt;script&gt;alert(&#39;literal&#39;)&lt;/script&gt;&#13;\n\t&amp;copy; 日本語 🦀"
    ));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("chosen?"));
    assert_eq!(post, before);
    for title in [false, true] {
        let mut invalid = post.clone();
        if title {
            invalid.title.push('\0');
        } else {
            invalid.body_text.push('\0');
        }
        assert_eq!(
            render_post(&invalid, usize::MAX),
            Err(PreparedPostsError::InvalidText)
        );
        assert_eq!(
            export_bundle(&[invalid], usize::MAX, usize::MAX),
            Err(PreparedPostsError::InvalidText)
        );
    }
}

#[test]
fn body_and_final_json_caps_measure_completed_bytes_and_include_terminal_lf() {
    let post = post();
    let body = render_post(&post, usize::MAX).unwrap();
    assert_eq!(render_post(&post, body.len()).unwrap(), body);
    assert_eq!(
        render_post(&post, body.len() - 1),
        Err(PreparedPostsError::OutputLimit)
    );
    let wire = export_bundle(std::slice::from_ref(&post), body.len(), usize::MAX).unwrap();
    assert!(wire.ends_with(b"\n"));
    assert!(wire.len() > body.len());
    assert_eq!(
        export_bundle(std::slice::from_ref(&post), body.len(), wire.len()).unwrap(),
        wire
    );
    assert_eq!(
        export_bundle(std::slice::from_ref(&post), body.len(), wire.len() - 1),
        Err(PreparedPostsError::OutputLimit)
    );
    assert_eq!(
        export_bundle(std::slice::from_ref(&post), body.len() - 1, usize::MAX),
        Err(PreparedPostsError::OutputLimit)
    );
    let input = serde_json::to_vec(&json!({"format_version":1,"posts":[{
        "target":post.target,"title":post.title,"body_text":post.body_text}]}))
    .unwrap();
    assert_eq!(
        prepare_bundle(&input, body.len(), wire.len()).unwrap(),
        wire
    );
    let exported: Value = serde_json::from_slice(&wire).unwrap();
    assert_eq!(exported["format_version"], 1);
    assert_eq!(exported["pages"][0]["target"], "/chosen?b=2&a=1");
    assert_eq!(
        exported["pages"][0]["body"].as_str().unwrap().as_bytes(),
        body
    );
    assert!(recastpress_server::bundle::parse(&wire).is_ok());
}

#[test]
fn final_admission_rejects_duplicates_reserved_targets_invalid_targets_and_empty_sites() {
    assert_eq!(
        export_bundle(&[], usize::MAX, usize::MAX),
        Err(PreparedPostsError::Admission)
    );
    let post = post();
    assert_eq!(
        export_bundle(&[post.clone(), post.clone()], usize::MAX, usize::MAX),
        Err(PreparedPostsError::Admission)
    );
    for target in [
        "/health/live",
        "/health/ready?x=1",
        "//foreign",
        "/has space",
        "/fragment#x",
    ] {
        let mut invalid = post.clone();
        invalid.target = target.into();
        assert_eq!(
            export_bundle(&[invalid], usize::MAX, usize::MAX),
            Err(PreparedPostsError::Admission)
        );
    }
    let mut empty = post;
    empty.title.clear();
    empty.body_text.clear();
    assert!(export_bundle(&[empty], usize::MAX, usize::MAX).is_ok());
}

#[test]
fn hard_bundle_bound_is_preserved_even_with_unlimited_caller_caps() {
    let mut post = PreparedPost {
        target: "/".into(),
        title: "".into(),
        body_text: "".into(),
    };
    let baseline = export_bundle(std::slice::from_ref(&post), usize::MAX, usize::MAX).unwrap();
    let limit = recastpress_server::bundle::MAX_BUNDLE_BYTES as usize;
    post.body_text = "x".repeat(limit - baseline.len());
    let exact = export_bundle(std::slice::from_ref(&post), usize::MAX, usize::MAX).unwrap();
    assert_eq!(exact.len(), limit);
    assert!(recastpress_server::bundle::parse(&exact).is_ok());
    post.body_text.push('x');
    assert_eq!(
        export_bundle(&[post], usize::MAX, usize::MAX),
        Err(PreparedPostsError::OutputLimit)
    );
}

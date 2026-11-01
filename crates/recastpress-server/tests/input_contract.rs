use recastpress_server::{
    bundle,
    cli::{self, Role},
};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}

#[test]
fn roles_and_options_are_explicit_and_separate() {
    let public = cli::parse(args(&[
        "public",
        "--host",
        "example.test",
        "--bundle",
        "site.json",
    ]))
    .unwrap();
    assert_eq!(public.role, Role::Public);
    assert_eq!(public.listen.to_string(), "127.0.0.1:8080");
    let management = cli::parse(args(&["management"])).unwrap();
    assert_eq!(management.listen.to_string(), "127.0.0.1:8081");
    assert!(management.bundle.is_none());
    for invalid in [
        vec![],
        vec!["public"],
        vec!["other"],
        vec!["management", "--bundle", "site.json"],
        vec!["management", "--host", "example.test"],
        vec!["public", "--host", "one", "--host", "two"],
        vec!["management", "--listen", "invalid"],
        vec!["management", "--listen"],
    ] {
        assert!(cli::parse(args(&invalid)).is_err(), "{invalid:?}");
    }
}

#[test]
fn bundle_rejects_unknown_versions_fields_duplicates_and_bad_pages() {
    for invalid in [
        r#"{"format_version":3,"pages":[]}"#,
        r#"{"format_version":1,"pages":[],"php":"run"}"#,
        r#"{"format_version":1,"format_version":1,"pages":[]}"#,
        r#"{"format_version":1,"pages":[]}"#,
        r#"{"format_version":1,"pages":[{"target":"/","status":200,"content_type":"text/html; charset=utf-8","body":"ok","private":true}]}"#,
        r#"{"format_version":1,"pages":[{"target":"//evil.test/","status":200,"content_type":"text/html; charset=utf-8","body":"ok"}]}"#,
        r#"{"format_version":1,"pages":[{"target":"/health/live?q=1","status":200,"content_type":"text/html; charset=utf-8","body":"ok"}]}"#,
        r#"{"format_version":1,"pages":[{"target":"/","status":204,"content_type":"text/html; charset=utf-8","body":"invalid body"}]}"#,
    ] {
        assert!(bundle::parse(invalid.as_bytes()).is_err(), "{invalid}");
    }
    assert!(bundle::parse(&vec![b' '; bundle::MAX_BUNDLE_BYTES as usize + 1]).is_err());
}

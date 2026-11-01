use recastpress_server::cli::{self, Role};
use std::path::PathBuf;
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn public_receipt_is_explicit_bundle_bound_and_order_independent_without_changing_legacy_roles() {
    for values in [
        vec![
            "public",
            "--host",
            "example.test",
            "--bundle",
            "prepared.json",
            "--receipt",
            "receipt.json",
        ],
        vec![
            "public",
            "--receipt",
            "receipt.json",
            "--host",
            "example.test",
            "--bundle",
            "prepared.json",
        ],
    ] {
        let config = cli::parse(args(&values)).unwrap();
        assert_eq!(config.role, Role::Public);
        assert_eq!(config.bundle, Some(PathBuf::from("prepared.json")));
        assert_eq!(config.receipt, Some(PathBuf::from("receipt.json")));
        assert_eq!(config.listen.to_string(), "127.0.0.1:8080");
    }
    let public = cli::parse(args(&["public", "--host", "example.test"])).unwrap();
    assert!(public.bundle.is_none());
    assert!(public.receipt.is_none());
    let legacy = cli::parse(args(&[
        "public",
        "--host",
        "example.test",
        "--bundle",
        "legacy.json",
    ]))
    .unwrap();
    assert!(legacy.receipt.is_none());
    let management = cli::parse(args(&["management"])).unwrap();
    assert_eq!(management.role, Role::Management);
    assert!(management.receipt.is_none());
}

#[test]
fn receipts_cannot_be_management_options_or_repeat_or_bypass_required_bundle_and_values() {
    for (values, description) in [
        (
            vec![
                "public",
                "--host",
                "example.test",
                "--receipt",
                "receipt.json",
            ],
            "receipt requires --bundle",
        ),
        (
            vec!["management", "--receipt", "receipt.json"],
            "management does not accept",
        ),
        (
            vec![
                "management",
                "--bundle",
                "prepared.json",
                "--receipt",
                "receipt.json",
            ],
            "management does not accept",
        ),
        (
            vec![
                "public",
                "--host",
                "example.test",
                "--bundle",
                "prepared.json",
                "--receipt",
                "one",
                "--receipt",
                "two",
            ],
            "unknown or repeated argument: --receipt",
        ),
        (
            vec![
                "public",
                "--host",
                "example.test",
                "--bundle",
                "prepared.json",
                "--receipt",
            ],
            "missing value for --receipt",
        ),
    ] {
        assert!(
            cli::parse(args(&values)).unwrap_err().contains(description),
            "{values:?}"
        );
    }
    // One-shots dispatch before the serve-role parser, not as new role variants.
    for command in [
        "prepare-posts",
        "bundle-receipt",
        "check-bundle",
        "inspect-memory",
    ] {
        assert!(cli::USAGE.contains(command));
        assert!(cli::parse(args(&[command])).is_err());
    }
}

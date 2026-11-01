use recastpress_wp::{LoginLookupPlan, PreparedLoginInput, SanitizeUserError};

#[test]
fn source_prefix_owns_s1_and_borrows_trimmed_raw_password_without_claiming_a_user() {
    let raw_username = b" \tL\xc3\xb3GIN%20&dummy;\x0b";
    let password = b" \t\0public\0\xffpassword\r\n\x0b";
    let prepared = PreparedLoginInput::new(raw_username, password, b"en_US", 64).unwrap();
    assert_eq!(prepared.username_stage1(), b"LoGIN");
    assert_eq!(prepared.email_lookup_key(), b"LoGIN");
    assert_eq!(prepared.password_trimmed(), b"public\0\xffpassword");
    assert_eq!(prepared.password_trimmed().as_ptr(), password[3..].as_ptr());
    assert!(!prepared.username_php_empty());
    assert!(!prepared.password_php_empty());
    assert!(
        matches!(prepared.login_lookup_plan().unwrap(),LoginLookupPlan::QueryKey(key) if key==b"LoGIN")
    );
}

#[test]
fn lookup_repeats_sanitize_after_trim_and_keeps_empty_second_result_as_query_key() {
    for input in [b"%&x;61".as_slice(), b"%4%30A"] {
        let prepared = PreparedLoginInput::new(input, b"password", b"en_US", 64).unwrap();
        assert!(!prepared.username_php_empty());
        assert!(
            matches!(prepared.login_lookup_plan().unwrap(),LoginLookupPlan::QueryKey(key) if key.is_empty())
        );
    }
    let prepared = PreparedLoginInput::new("l&x;·l".as_bytes(), b"password", b"ca", 64).unwrap();
    assert_eq!(prepared.username_stage1(), "l·l".as_bytes());
    assert!(
        matches!(prepared.login_lookup_plan().unwrap(),LoginLookupPlan::QueryKey(key) if key==b"ll")
    );
}

#[test]
fn php_empty_flags_are_stage_specific_and_lookup_trim_can_skip_nonempty_s1() {
    let ff = PreparedLoginInput::new(b"\x0c", b"0", b"en_US", 1).unwrap();
    assert_eq!(ff.username_stage1(), b" ");
    assert!(!ff.username_php_empty());
    assert!(ff.password_php_empty());
    assert!(matches!(
        ff.login_lookup_plan().unwrap(),
        LoginLookupPlan::Skipped
    ));
    let zero = PreparedLoginInput::new(b"&x;0", b" \0\t", b"en_US", 1).unwrap();
    assert!(zero.username_php_empty());
    assert!(zero.password_php_empty());
    assert!(matches!(
        zero.login_lookup_plan().unwrap(),
        LoginLookupPlan::Skipped
    ));
    let password = PreparedLoginInput::new(b"user", b"\x0c0\x0c", b"en_US", 4).unwrap();
    assert!(!password.password_php_empty());
}

#[test]
fn debug_redacts_all_prepared_and_query_payloads_and_domain_caps_stay_explicit() {
    let input = PreparedLoginInput::new(
        b"PRIVATE-email@example.invalid",
        b"PRIVATE-password",
        b"PRIVATE-locale",
        64,
    )
    .unwrap();
    assert_eq!(format!("{input:?}"), "PreparedLoginInput([redacted])");
    let plan = input.login_lookup_plan().unwrap();
    assert_eq!(format!("{plan:?}"), "LoginLookupPlan([redacted])");
    assert!(matches!(
        PreparedLoginInput::new(b"user", b"password", b"en_US", 3),
        Err(SanitizeUserError::OutputLimit)
    ));
    assert!(matches!(
        PreparedLoginInput::new(b"<user", b"password", b"en_US", 64),
        Err(SanitizeUserError::UnsupportedValue)
    ));
}

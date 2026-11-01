use recastpress_wp::auth_cookie_parse::{
    AuthCookieParseError as Error, wp_parse_auth_cookie_explicit_bytes as parse,
};

#[test]
fn exact_three_pipes_accept_four_raw_fields_including_all_empty_or_unvalidated_values() {
    let empty = parse(b"|||", b"", 3).unwrap().unwrap();
    assert!(empty.username().is_empty());
    assert!(empty.expiration_bytes().is_empty());
    assert!(empty.token().is_empty());
    assert!(empty.hmac_bytes().is_empty());
    assert!(empty.scheme_label().is_empty());
    for (raw, expected_expiry, expected_token, expected_hmac) in [
        (
            b"0|0001|0|!".as_slice(),
            b"0001".as_slice(),
            b"0".as_slice(),
            b"!".as_slice(),
        ),
        (b"0|-1||not-hex", b"-1", b"", b"not-hex"),
        (b"0|1e999|token|", b"1e999", b"token", b""),
        (b"0|abc||0", b"abc", b"", b"0"),
    ] {
        let value = parse(raw, b"caller-label", usize::MAX).unwrap().unwrap();
        assert_eq!(value.username(), b"0");
        assert_eq!(value.expiration_bytes(), expected_expiry);
        assert_eq!(value.token(), expected_token);
        assert_eq!(value.hmac_bytes(), expected_hmac);
    }
}

#[test]
fn any_other_field_count_is_source_false_including_an_extra_trailing_delimiter() {
    for cookie in [
        b"plain".as_slice(),
        b"|",
        b"||",
        b"a|b|c",
        b"a|b|c|d|",
        b"||||",
        b"a|b|c|d|e|f",
    ] {
        assert!(parse(cookie, b"arbitrary", usize::MAX).unwrap().is_none());
    }
    assert!(parse(b"a|b|c|d", b"", usize::MAX).unwrap().is_some());
    assert!(parse(b"|b|c|d", b"", usize::MAX).unwrap().is_some());
    assert!(parse(b"a|b|c|", b"", usize::MAX).unwrap().is_some());
}

#[test]
fn explicit_branch_refusals_precede_caps_and_cap_counts_the_complete_scheme_label() {
    for cookie in [b"".as_slice(), b"0"] {
        assert_eq!(
            parse(cookie, b"label", 0).unwrap_err(),
            Error::UnsupportedValue
        );
        assert_eq!(
            parse(cookie, b"label", usize::MAX).unwrap_err(),
            Error::UnsupportedValue
        );
    }
    let cookie = b"a|b|c|d";
    let scheme = b"raw|label\0\xff";
    let exact = cookie.len() + scheme.len();
    assert!(parse(cookie, scheme, exact).unwrap().is_some());
    assert_eq!(
        parse(cookie, scheme, exact - 1).unwrap_err(),
        Error::InputLimit
    );
    assert_eq!(parse(cookie, scheme, 0).unwrap_err(), Error::InputLimit);
    // The cap is checked even if field counting would later yield false.
    assert_eq!(parse(b"too|few", scheme, 0).unwrap_err(), Error::InputLimit);
}

#[test]
fn binary_unicode_and_arbitrary_scheme_labels_are_borrowed_without_normalization() {
    let cookie = b"\0\xffuser|\r\nabc\0|\xc3\xa9token|\x80digest";
    let before = cookie.to_vec();
    for scheme in [
        b"".as_slice(),
        b"0",
        b"auth",
        b"secure_auth",
        b"logged_in",
        b"UNKNOWN|\0\xff",
    ] {
        let value = parse(cookie, scheme, usize::MAX).unwrap().unwrap();
        assert_eq!(value.username(), b"\0\xffuser");
        assert_eq!(value.expiration_bytes(), b"\r\nabc\0");
        assert_eq!(value.token(), "étoken".as_bytes());
        assert_eq!(value.hmac_bytes(), b"\x80digest");
        assert_eq!(value.scheme_label(), scheme);
        assert_eq!(value.username().as_ptr(), cookie.as_ptr());
        assert_eq!(value.expiration_bytes().as_ptr(), cookie[7..].as_ptr());
        assert_eq!(value.token().as_ptr(), cookie[14..].as_ptr());
        assert_eq!(value.hmac_bytes().as_ptr(), cookie[22..].as_ptr());
        assert_eq!(value.scheme_label().as_ptr(), scheme.as_ptr());
    }
    assert_eq!(cookie.as_slice(), before);
}

#[test]
fn parsed_view_and_errors_never_debug_raw_client_fields_or_scheme() {
    let value = parse(b"PRIVATE|PRIVATE|PRIVATE|PRIVATE", b"PRIVATE", usize::MAX)
        .unwrap()
        .unwrap();
    assert_eq!(format!("{value:?}"), "ParsedAuthCookie([redacted])");
    for error in [Error::UnsupportedValue, Error::InputLimit] {
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[cfg(feature = "auth")]
#[test]
fn generated_cookie_relation_requires_delimiter_free_login_and_token_without_claiming_validation() {
    use recastpress_wp::{
        AuthCookieLimits, PreparedAuthCookieUser,
        wp_generate_auth_cookie_prepared_resolved_salt_empty_filters_bytes as generate,
    };
    let limits = AuthCookieLimits::new(usize::MAX, usize::MAX);
    let user = PreparedAuthCookieUser::new(1, b"public-user", b"public-hash").unwrap();
    let generated = generate(&user, -1, b"public-token", b"public-salt", limits).unwrap();
    let value = parse(generated.as_cookie_bytes(), b"unchanged-label", usize::MAX)
        .unwrap()
        .unwrap();
    assert_eq!(value.username(), b"public-user");
    assert_eq!(value.expiration_bytes(), b"-1");
    assert_eq!(value.token(), b"public-token");
    assert_eq!(value.hmac_bytes().len(), 64);
    assert_eq!(value.scheme_label(), b"unchanged-label");
    for (login, token) in [
        (b"user|extra".as_slice(), b"token".as_slice()),
        (b"user", b"token|extra"),
    ] {
        let user = PreparedAuthCookieUser::new(1, login, b"public-hash").unwrap();
        let generated = generate(&user, 0, token, b"public-salt", limits).unwrap();
        assert!(
            parse(generated.as_cookie_bytes(), b"auth", usize::MAX)
                .unwrap()
                .is_none()
        );
    }
}

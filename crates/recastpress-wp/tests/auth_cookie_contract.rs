use recastpress_wp::{
    AuthCookieError as Error, AuthCookieLimits, PreparedAuthCookieUser, WpAuthCookie,
    wp_generate_auth_cookie_prepared_resolved_salt_empty_filters_bytes as generate,
};

fn limits() -> AuthCookieLimits {
    AuthCookieLimits::new(usize::MAX, usize::MAX)
}

fn cookie(hash: &[u8]) -> WpAuthCookie {
    let user = PreparedAuthCookieUser::new(1, b"public-user", hash).unwrap();
    generate(
        &user,
        1700000000,
        b"public-token",
        b"public-resolved-salt",
        limits(),
    )
    .unwrap()
}

#[test]
fn default_two_hmac_pipeline_uses_lowercase_md5_hex_ascii_key_and_raw_storage_framing() {
    // Independent Python stdlib HMAC-MD5/HMAC-SHA256 public dummy observations;
    // these are not claims of a PHP differential or session authentication.
    let observed = cookie(b"$wp$2y$12$public-dummy-hash-ABCD");
    assert_eq!(observed.as_cookie_bytes(), b"public-user|1700000000|public-token|e6d181784450dc092231e47c479684a469ee8316d8f48b5c89aac79974b8d8d8");
    let user = PreparedAuthCookieUser::new(1, b"public", b"hash").unwrap();
    assert_eq!(
        generate(&user, 0, b"token", &[b's'; 100], limits())
            .unwrap()
            .as_cookie_bytes(),
        b"public|0|token|295a6cea28e8d72ec93523de991c9688151cde2941fab2414a576bae8327b91c"
    );
}

#[test]
fn hash_prefix_fragment_selection_is_byte_based_even_for_short_or_malformed_hashes() {
    let reference = cookie(b"anythingABCD");
    for hash in [
        b"$P$12345ABCDignored".as_slice(),
        b"$2y$1234ABCDdifferent",
        b"$wp$2y$12$garbageABCD",
        b"ABCD",
    ] {
        assert_eq!(cookie(hash).as_cookie_bytes(), reference.as_cookie_bytes());
    }
    let empty = cookie(b"");
    for hash in [b"$P$".as_slice(), b"$P$12345", b"$2y$", b"$2y$1234"] {
        assert_eq!(cookie(hash).as_cookie_bytes(), empty.as_cookie_bytes());
    }
    for (prefixed, plain) in [
        (b"$P$12345A".as_slice(), b"A".as_slice()),
        (b"$2y$1234AB", b"AB"),
        (b"$P$12345\0\xff", b"\0\xff"),
    ] {
        assert_eq!(
            cookie(prefixed).as_cookie_bytes(),
            cookie(plain).as_cookie_bytes()
        );
    }
    // Case/prefix variants do NOT enter the phpass/raw-2y slicing branch.
    assert_eq!(
        cookie(b"$p$12345ABCDtail").as_cookie_bytes(),
        cookie(b"tail").as_cookie_bytes()
    );
    assert_eq!(
        cookie(b"$2b$1234ABCDtail").as_cookie_bytes(),
        cookie(b"tail").as_cookie_bytes()
    );
    assert_ne!(
        cookie(b"$2y$1234ABCDtail").as_cookie_bytes(),
        cookie(b"tail").as_cookie_bytes()
    );
}

#[test]
fn signed_expiration_extremes_and_binary_login_token_salt_are_preserved_without_trim_or_header_rules()
 {
    let user = PreparedAuthCookieUser::new(i64::MAX, b"public", b"hash").unwrap();
    for (expiration, expected) in [
        (i64::MIN, b"public|-9223372036854775808|token|74676e1181e5e55af0b627317ddbfc7db12565c10b234189c48cedea631f05c5".as_slice()),
        (i64::MAX, b"public|9223372036854775807|token|1f25f196b85c5a35de5ef92a296c2e1eb0bc39ff6f3b5a91208624773dea731c"),
    ] { assert_eq!(generate(&user, expiration, b"token", b"salt", limits()).unwrap().as_cookie_bytes(), expected); }
    let login = b"\0\xffpub|user\r\n";
    let hash = b"tiny";
    let user = PreparedAuthCookieUser::new(1, login, hash).unwrap();
    assert_eq!(generate(&user, -1, b"\0\x80token|x", b"\0publicsalt\xff", limits()).unwrap().as_cookie_bytes(), b"\0\xffpub|user\r\n|-1|\0\x80token|x|e51dccc9adcdc199a6abd5cc50dbd99f1eb03885e65fc3e50c5b7f32c25566c6");
    assert_eq!(login, b"\0\xffpub|user\r\n");
    assert_eq!(hash, b"tiny");
    let empty_fields = PreparedAuthCookieUser::new(1, b"", b"").unwrap();
    assert_eq!(
        generate(&empty_fields, 0, b"t", b"0", limits())
            .unwrap()
            .as_cookie_bytes(),
        b"|0|t|73a74e33e2d11bc5c623a0a00c53db9fc98b0da479530df22d866d1addc3552c"
    );
}

#[test]
fn mandatory_total_input_and_complete_output_caps_are_exact_and_charge_the_full_hash() {
    let login = b"public";
    let hash = b"$P$12345ABCDignored-password-hash-tail";
    let token = b"token";
    let salt = b"salt";
    let user = PreparedAuthCookieUser::new(1, login, hash).unwrap();
    let input = login.len() + hash.len() + token.len() + salt.len();
    let output = login.len() + 1 + token.len() + 67; // decimal expiration '0'.
    let exact = generate(&user, 0, token, salt, AuthCookieLimits::new(input, output)).unwrap();
    assert_eq!(exact.as_cookie_bytes().len(), output);
    assert_eq!(
        generate(
            &user,
            0,
            token,
            salt,
            AuthCookieLimits::new(input - 1, output)
        )
        .unwrap_err(),
        Error::InputLimit
    );
    assert_eq!(
        generate(
            &user,
            0,
            token,
            salt,
            AuthCookieLimits::new(input, output - 1)
        )
        .unwrap_err(),
        Error::OutputLimit
    );
    assert_eq!(
        generate(&user, 0, token, salt, AuthCookieLimits::new(0, 0)).unwrap_err(),
        Error::InputLimit
    );
    assert_eq!(
        generate(&user, 0, token, salt, limits())
            .unwrap()
            .as_cookie_bytes(),
        exact.as_cookie_bytes()
    );
    // Unused hash bytes still consume the raw input allowance.
    let mut long_hash = b"$P$12345ABCD".to_vec();
    long_hash.extend_from_slice(&[b'x'; 1024]);
    let long_user = PreparedAuthCookieUser::new(1, login, &long_hash).unwrap();
    assert_eq!(
        generate(
            &long_user,
            0,
            token,
            salt,
            AuthCookieLimits::new(input, output)
        )
        .unwrap_err(),
        Error::InputLimit
    );
    assert_eq!(
        generate(&long_user, 0, token, salt, limits())
            .unwrap()
            .as_cookie_bytes(),
        exact.as_cookie_bytes()
    );
}

#[test]
fn positive_user_and_nonempty_salt_php_truthy_token_are_profile_guards_before_caps() {
    for id in [i64::MIN, -1, 0] {
        assert_eq!(
            PreparedAuthCookieUser::new(id, b"public", b"hash").unwrap_err(),
            Error::UnsupportedValue
        );
    }
    let user = PreparedAuthCookieUser::new(1, b"public", b"hash").unwrap();
    for (token, salt) in [
        (b"".as_slice(), b"salt".as_slice()),
        (b"0", b"salt"),
        (b"token", b""),
    ] {
        assert_eq!(
            generate(&user, 0, token, salt, AuthCookieLimits::new(0, 0)).unwrap_err(),
            Error::UnsupportedValue
        );
    }
    for token in [b"00".as_slice(), b"\0", b" ", b"false", b"|\r\n"] {
        assert!(generate(&user, 0, token, b"0", limits()).is_ok());
    }
}

#[test]
fn cookie_owner_is_independent_and_debug_and_errors_never_expose_user_or_cookie_secrets() {
    let output = {
        let login = b"PRIVATE-user".to_vec();
        let hash = b"PRIVATE-hash".to_vec();
        let user = PreparedAuthCookieUser::new(1, &login, &hash).unwrap();
        assert_eq!(format!("{user:?}"), "PreparedAuthCookieUser([redacted])");
        generate(&user, 1, b"PRIVATE-token", b"PRIVATE-salt", limits()).unwrap()
    };
    assert!(
        output
            .as_cookie_bytes()
            .starts_with(b"PRIVATE-user|1|PRIVATE-token|")
    );
    assert_eq!(format!("{output:?}"), "WpAuthCookie([redacted])");
    assert!(!format!("{:?}", limits()).contains("PRIVATE"));
    for error in [
        Error::UnsupportedValue,
        Error::InputLimit,
        Error::OutputLimit,
        Error::AllocationFailed,
        Error::ProviderFailure,
    ] {
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

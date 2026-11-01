use super::*;
use super::{
    CookieRejectionStage as Stage, CookieSessionExpiration as SessionExpiration,
    CookieVerificationError as Error, CookieVerificationLimits as VerifyLimits,
    CookieVerificationOutcome as Outcome, PreparedCookieSession as PreparedSession,
    PreparedCookieSessions as PreparedSessions, PreparedCookieVerificationUser as PreparedUser,
    wp_validate_auth_cookie_prepared_get_non_ajax_resolved_salt_bytes as check,
};

use crate::{
    AuthCookieLimits, PreparedAuthCookieUser,
    wp_generate_auth_cookie_prepared_resolved_salt_empty_filters_bytes as generate,
};
fn user() -> PreparedUser<'static> {
    PreparedUser::new(7, b"canonical-row", b"public-password-hash").unwrap()
}
fn generated(login: &[u8], expiry: i64, token: &[u8]) -> Vec<u8> {
    let u = PreparedAuthCookieUser::new(7, login, b"public-password-hash").unwrap();
    generate(
        &u,
        expiry,
        token,
        b"public-salt",
        AuthCookieLimits::new(65536, 65536),
    )
    .unwrap()
    .as_cookie_bytes()
    .to_vec()
}
fn stage(value: Result<Outcome<'_>, Error>) -> Option<Stage> {
    match value.unwrap() {
        Outcome::NoMatch(s) => Some(s),
        Outcome::Match(_) => None,
    }
}
fn limits() -> VerifyLimits {
    VerifyLimits::new(65536)
}
#[test]
fn composition_uses_raw_cookie_username_and_same_borrowed_record_not_canonical_row_login() {
    let u = user();
    let key = token_verifier(b"public-token");
    let entries = [PreparedSession {
        verifier: &key,
        expiration: SessionExpiration::Record(100),
    }];
    let sessions = PreparedSessions::new(7, &entries, 1, 64).unwrap();
    let cookie = generated(b"RAW-user", 100, b"public-token");
    let Outcome::Match(proof) = check(
        &cookie,
        b"unknown\0",
        Some(&u),
        &sessions,
        b"public-salt",
        100,
        limits(),
    )
    .unwrap() else {
        panic!("expected match")
    };
    assert!(std::ptr::eq(proof._user, &u));
    assert_eq!(proof.prepared_record_id(), 7);
    for tag in [
        format!("{proof:?}"),
        format!("{u:?}"),
        format!("{sessions:?}"),
    ] {
        assert!(!tag.contains("RAW") && !tag.contains("password"));
    }
}
#[test]
fn explicit_now_equality_plus_minus_one_and_integer_extremes_cover_cookie_and_session_separately() {
    let u = user();
    let key = token_verifier(b"t");
    for expiry in [i64::MIN, -1, 0, 1, i64::MAX] {
        let cookie = generated(b"u", expiry, b"t");
        for shape in [
            SessionExpiration::Legacy(expiry),
            SessionExpiration::Record(expiry),
        ] {
            let entries = [PreparedSession {
                verifier: &key,
                expiration: shape,
            }];
            let sessions = PreparedSessions::new(7, &entries, 1, 64).unwrap();
            assert_eq!(
                stage(check(
                    &cookie,
                    b"",
                    Some(&u),
                    &sessions,
                    b"public-salt",
                    expiry,
                    limits()
                )),
                None
            );
            if let Some(after) = expiry.checked_add(1) {
                assert_eq!(
                    stage(check(
                        &cookie,
                        b"",
                        Some(&u),
                        &sessions,
                        b"public-salt",
                        after,
                        limits()
                    )),
                    Some(Stage::Expired)
                );
            }
            if let Some(before) = expiry.checked_sub(1) {
                assert_eq!(
                    stage(check(
                        &cookie,
                        b"",
                        Some(&u),
                        &sessions,
                        b"public-salt",
                        before,
                        limits()
                    )),
                    None
                );
            }
        }
    }
    // Native explicit-clock boundaries only. A future cookie prevents the
    // cookie quick-expiry branch from masking the separate session boundary.
    let cookie = generated(b"u", i64::MAX, b"t");
    for expiry in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert_eq!(
            canonical_i64(expiry.to_string().as_bytes()).unwrap(),
            expiry
        );
        for expiration in [
            SessionExpiration::Legacy(expiry),
            SessionExpiration::Record(expiry),
        ] {
            let entries = [PreparedSession {
                verifier: &key,
                expiration,
            }];
            let sessions = PreparedSessions::new(7, &entries, 1, 64).unwrap();
            for now in [Some(expiry), expiry.checked_sub(1), expiry.checked_add(1)]
                .into_iter()
                .flatten()
            {
                let expected = (now > expiry).then_some(Stage::BadSession);
                assert_eq!(
                    stage(check(
                        &cookie,
                        b"",
                        Some(&u),
                        &sessions,
                        b"public-salt",
                        now,
                        limits()
                    )),
                    expected
                );
            }
        }
    }
}

#[test]
fn independent_field_salt_hash_tampering_and_lowercase_tag_semantics_are_definite_mac_failures() {
    let u = user();
    let key = token_verifier(b"t");
    let e = [PreparedSession {
        verifier: &key,
        expiration: SessionExpiration::Record(100),
    }];
    let s = PreparedSessions::new(7, &e, 1, 64).unwrap();
    let cookie = generated(b"raw", 100, b"t");
    for field in 0..4 {
        let mut fields: Vec<Vec<u8>> = cookie.split(|b| *b == b'|').map(<[u8]>::to_vec).collect();
        if field == 1 {
            fields[field] = b"101".to_vec();
        } else {
            fields[field].push(b'x');
        }
        let changed = fields.join(&b'|');
        assert_eq!(
            stage(check(
                &changed,
                b"",
                Some(&u),
                &s,
                b"public-salt",
                0,
                limits()
            )),
            Some(Stage::BadMac)
        );
    }
    assert_eq!(
        stage(check(
            &cookie,
            b"",
            Some(&u),
            &s,
            b"wrong-salt",
            0,
            limits()
        )),
        Some(Stage::BadMac)
    );
    let other = PreparedUser::new(7, b"canonical", b"DIFFERENT-END").unwrap();
    assert_eq!(
        stage(check(
            &cookie,
            b"",
            Some(&other),
            &s,
            b"public-salt",
            0,
            limits()
        )),
        Some(Stage::BadMac)
    );
    let mut uppercase = cookie.clone();
    let at = uppercase.iter().rposition(|b| *b == b'|').unwrap() + 1;
    uppercase[at..].make_ascii_uppercase();
    assert_eq!(
        stage(check(
            &uppercase,
            b"",
            Some(&u),
            &s,
            b"public-salt",
            0,
            limits()
        )),
        Some(Stage::BadMac)
    );
}
#[test]
fn source_false_order_is_separate_from_complete_preflight_refusals() {
    let u = user();
    let s = PreparedSessions::new(7, &[], 0, 0).unwrap();
    assert_eq!(
        stage(check_with(
            b"plain",
            b"",
            Some(&u),
            &s,
            b"salt",
            1,
            limits(),
            |_| panic!("MAC after malformed")
        )),
        Some(Stage::Malformed)
    );
    assert_eq!(
        stage(check_with(
            b"u|0|t|bad",
            b"",
            Some(&u),
            &s,
            b"salt",
            1,
            limits(),
            |_| panic!("MAC after expired")
        )),
        Some(Stage::Expired)
    );
    assert_eq!(
        stage(check_with(
            b"u|1|t|bad",
            b"",
            None,
            &s,
            b"salt",
            1,
            limits(),
            |_| panic!("MAC after unknown user")
        )),
        Some(Stage::UnknownUser)
    );
    assert_eq!(
        stage(check_with(
            b"u|1|t|bad",
            b"",
            Some(&u),
            &s,
            b"salt",
            1,
            limits(),
            |_| Ok(false)
        )),
        Some(Stage::BadMac)
    );
    assert!(matches!(
        check_with(
            b"u|1|t|bad",
            b"",
            Some(&u),
            &s,
            b"salt",
            1,
            limits(),
            |_| Err(Error::ProviderFailure)
        ),
        Err(Error::ProviderFailure)
    ));
    let good = token_verifier(b"unused");
    let e = [
        PreparedSession {
            verifier: &good,
            expiration: SessionExpiration::Record(100),
        },
        PreparedSession {
            verifier: b"PRIVATE-bad-unselected",
            expiration: SessionExpiration::Legacy(100),
        },
    ];
    assert!(matches!(
        PreparedSessions::new(7, &e, 2, 128),
        Err(Error::InvalidSession)
    ));
}
#[test]
fn mandatory_caps_all_entries_duplicate_owner_and_canonical_expiry_refuse_without_false_results() {
    let u = user();
    let key = token_verifier(b"t");
    let e = [PreparedSession {
        verifier: &key,
        expiration: SessionExpiration::Legacy(10),
    }];
    let s = PreparedSessions::new(7, &e, 1, 64).unwrap();
    let cookie = generated(b"u", 10, b"t");
    let exact =
        cookie.len() + b"scheme".len() + b"public-salt".len() + u.login.len() + u.hash.len() + 64;
    assert_eq!(
        stage(check(
            &cookie,
            b"scheme",
            Some(&u),
            &s,
            b"public-salt",
            10,
            VerifyLimits::new(exact)
        )),
        None
    );
    assert!(matches!(
        check(
            &cookie,
            b"scheme",
            Some(&u),
            &s,
            b"public-salt",
            10,
            VerifyLimits::new(exact - 1)
        ),
        Err(Error::InputLimit)
    ));
    assert!(matches!(
        PreparedSessions::new(7, &e, 0, 64),
        Err(Error::SessionLimit)
    ));
    assert!(matches!(
        PreparedSessions::new(7, &e, 1, 63),
        Err(Error::SessionLimit)
    ));
    let duplicate = [
        PreparedSession {
            verifier: &key,
            expiration: SessionExpiration::Legacy(1),
        },
        PreparedSession {
            verifier: &key,
            expiration: SessionExpiration::Record(2),
        },
    ];
    assert!(matches!(
        PreparedSessions::new(7, &duplicate, 2, 128),
        Err(Error::InvalidSession)
    ));
    let other = PreparedSessions::new(8, &e, 1, 64).unwrap();
    assert!(matches!(
        check(&cookie, b"", Some(&u), &other, b"public-salt", 10, limits()),
        Err(Error::UnsupportedValue)
    ));
    for value in [
        b"".as_slice(),
        b"+1",
        b"01",
        b"-0",
        b"1.0",
        b"1e2",
        b"9223372036854775808",
        b"-9223372036854775809",
    ] {
        assert_eq!(canonical_i64(value), Err(Error::UnsupportedValue));
    }
    for raw in [b"".as_slice(), b"0"] {
        assert!(matches!(
            check(raw, b"", Some(&u), &s, b"salt", 0, limits()),
            Err(Error::UnsupportedValue)
        ));
    }
    assert!(PreparedUser::new(0, b"u", b"h").is_err());
}
#[test]
fn empty_zero_tokens_are_verified_without_generation_guard_and_pipe_generation_is_malformed() {
    // Public expected values from independent Python stdlib HMAC fixture; not generated by this verifier.
    for (token, cookie) in [
        (
            b"".as_slice(),
            b"u|10||61e863d0fb47179c5ef662a920c8d8ab2509bfac5372a08ad1ead8d75516c123".as_slice(),
        ),
        (
            b"0".as_slice(),
            b"u|10|0|b8e3acff3ad7f3e5077672adfa6b22e556f9d412c9fe232e4844533056dd3de2".as_slice(),
        ),
    ] {
        let u = user();
        let key = token_verifier(token);
        let e = [PreparedSession {
            verifier: &key,
            expiration: SessionExpiration::Legacy(10),
        }];
        let s = PreparedSessions::new(7, &e, 1, 64).unwrap();
        assert_eq!(
            stage(check(
                cookie,
                b"",
                Some(&u),
                &s,
                b"public-salt",
                10,
                limits()
            )),
            None
        );
    }
    let u = user();
    let s = PreparedSessions::new(7, &[], 0, 0).unwrap();
    for (login, token) in [(b"u|x".as_slice(), b"t".as_slice()), (b"u", b"t|x")] {
        assert_eq!(
            stage(check(
                &generated(login, 10, token),
                b"",
                Some(&u),
                &s,
                b"public-salt",
                0,
                limits()
            )),
            Some(Stage::Malformed)
        );
    }
}

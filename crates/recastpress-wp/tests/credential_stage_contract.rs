use recastpress_wp::{
    CredentialLookup, CredentialLookupKind, CredentialPhase, CredentialStage,
    CredentialStageOutcome, CredentialUnresolvedReason, HostMaxBcryptCost, HostMaxPhpassCost,
    HostPasswordWorkLimits, PreparedLoginInput, RehashIntent,
};
use std::{cell::Cell, rc::Rc};

// Intentionally no Debug/Clone bound: generic stages must never format R.
struct Record {
    id: u64,
    hash: Vec<u8>,
    reads: Rc<Cell<usize>>,
}
fn stored_hash(record: &Record) -> &[u8] {
    record.reads.set(record.reads.get() + 1);
    &record.hash
}
fn record(id: u64, hash: &[u8]) -> Record {
    Record {
        id,
        hash: hash.to_vec(),
        reads: Rc::new(Cell::new(0)),
    }
}
fn begin<'a>(username: &[u8], password: &'a [u8]) -> CredentialStage<'a, Record> {
    CredentialStage::begin(
        PreparedLoginInput::new(username, password, b"en_US", 1024).unwrap(),
        HostPasswordWorkLimits::new(
            HostMaxBcryptCost::new(4).unwrap(),
            HostMaxPhpassCost::new(7).unwrap(),
        ),
        stored_hash,
    )
}
const PASSWORD: &[u8] = b"public-owned-rehash-dummy";
const HASH: &[u8] = b"1353bb1343967c24b73dd0e089c998ae";

#[test]
fn matching_username_stops_before_email_and_retains_the_exact_opaque_raw_record_once() {
    let CredentialStage::NeedLogin(request) = begin(b"member@example.invalid", PASSWORD) else {
        panic!("login request required")
    };
    assert_eq!(request.query_key(), b"member@example.invalid");
    let row = record(u64::MAX, HASH);
    let reads = row.reads.clone();
    let CredentialStage::Complete(CredentialStageOutcome::Match(candidate)) =
        request.respond(CredentialLookup::Found(row))
    else {
        panic!("username match must be terminal")
    };
    assert_eq!(candidate.record().id, u64::MAX);
    assert!(candidate.matched_via() == CredentialLookupKind::Login);
    assert!(candidate.rehash_intent() == RehashIntent::Suggested);
    assert_eq!(reads.get(), 1);
}

#[test]
fn only_definite_username_absence_or_mismatch_uses_s1_email_then_trim_only() {
    for found_wrong in [false, true] {
        let CredentialStage::NeedLogin(request) = begin(b"%&x;31@example.invalid", PASSWORD) else {
            panic!("login request required")
        };
        assert_eq!(request.query_key(), b"@example.invalid"); // S2
        let reply = if found_wrong {
            CredentialLookup::Found(record(19, b"00000000000000000000000000000000"))
        } else {
            CredentialLookup::NotFound
        };
        let CredentialStage::NeedEmail(request) = request.respond(reply) else {
            panic!("S1 is an email candidate")
        };
        assert_eq!(request.query_key(), b"%31@example.invalid"); // S1, not S2
        let CredentialStage::Complete(CredentialStageOutcome::Match(candidate)) =
            request.respond(CredentialLookup::Found(record(0, HASH)))
        else {
            panic!("email credential match required")
        };
        assert_eq!(candidate.record().id, 0); // raw ID, never an auth principal cast
        assert!(candidate.matched_via() == CredentialLookupKind::Email);
    }
}

#[test]
fn lookup_errors_unsupported_formats_and_both_cost_limits_are_terminal_unresolved() {
    for (hash, reason) in [
        (
            b"unknown-non-prefixed-hash-over-32-bytes".as_slice(),
            CredentialUnresolvedReason::UnsupportedHash,
        ),
        (
            b"$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C",
            CredentialUnresolvedReason::BcryptCostLimit,
        ),
        (
            b"$P$B12345678ymVq6hVWkVcT1b2.4Yw6B.",
            CredentialUnresolvedReason::PortableCostLimit,
        ),
    ] {
        let CredentialStage::NeedLogin(request) = begin(b"member@example.invalid", PASSWORD) else {
            panic!("login request required")
        };
        assert!(
            matches!(request.respond(CredentialLookup::Found(record(1,hash))),CredentialStage::Complete(CredentialStageOutcome::Unresolved{at:CredentialPhase::Login,reason:found}) if found==reason)
        );
    }
    let CredentialStage::NeedLogin(request) = begin(b"member@example.invalid", PASSWORD) else {
        panic!("login request required")
    };
    assert!(matches!(
        request.respond(CredentialLookup::LookupUnresolved),
        CredentialStage::Complete(CredentialStageOutcome::Unresolved {
            at: CredentialPhase::Login,
            reason: CredentialUnresolvedReason::Lookup
        })
    ));
}

#[test]
fn input_empty_and_skipped_are_distinct_from_required_empty_or_zero_s2_queries() {
    for (username, password) in [
        (b"0".as_slice(), PASSWORD),
        (b"member@example.invalid", b" \0\t"),
        (b"member@example.invalid", b"0"),
        (b"\x0c", PASSWORD),
    ] {
        assert!(matches!(
            begin(username, password),
            CredentialStage::Complete(CredentialStageOutcome::NoCredentialMatch)
        ));
    }
    for (username, key) in [(b"%&x;61".as_slice(), b"".as_slice()), (b"%&x;310", b"0")] {
        let CredentialStage::NeedLogin(request) = begin(username, PASSWORD) else {
            panic!("no post-S2 empty check")
        };
        assert_eq!(request.query_key(), key);
    }
}

#[test]
fn email_absence_mismatch_and_lookup_failure_never_restart_login_or_claim_a_principal() {
    for reply in [
        CredentialLookup::NotFound,
        CredentialLookup::Found(record(2, b"00000000000000000000000000000000")),
        CredentialLookup::LookupUnresolved,
    ] {
        let CredentialStage::NeedLogin(request) = begin(b"member@example.invalid", PASSWORD) else {
            panic!("login request required")
        };
        let CredentialStage::NeedEmail(request) = request.respond(CredentialLookup::NotFound)
        else {
            panic!("email request required")
        };
        let expected_unresolved = matches!(&reply, CredentialLookup::LookupUnresolved);
        let result = request.respond(reply);
        if expected_unresolved {
            assert!(matches!(
                result,
                CredentialStage::Complete(CredentialStageOutcome::Unresolved {
                    at: CredentialPhase::Email,
                    reason: CredentialUnresolvedReason::Lookup
                })
            ));
        } else {
            assert!(matches!(
                result,
                CredentialStage::Complete(CredentialStageOutcome::NoCredentialMatch)
            ));
        }
    }
    let CredentialStage::NeedLogin(request) = begin(b"not-an-email", PASSWORD) else {
        panic!("login request required")
    };
    assert!(matches!(
        request.respond(CredentialLookup::NotFound),
        CredentialStage::Complete(CredentialStageOutcome::NoCredentialMatch)
    ));
}

#[test]
fn raw_long_password_keeps_md5_before_length_guard_and_all_state_debug_is_redacted() {
    let password = vec![b'x'; 4097];
    let CredentialStage::NeedLogin(request) = begin(b"member@example.invalid", &password) else {
        panic!("login request required")
    };
    assert_eq!(format!("{request:?}"), "LoginLookupRequest([redacted])");
    let reply = CredentialLookup::Found(record(7, b"88751dd296563726278c2d57ab7549e4"));
    assert_eq!(format!("{reply:?}"), "CredentialLookup([redacted])");
    let result = request.respond(reply);
    assert_eq!(format!("{result:?}"), "CredentialStage([redacted])");
    let CredentialStage::Complete(outcome) = result else {
        panic!("long MD5 match is terminal")
    };
    assert_eq!(format!("{outcome:?}"), "CredentialStageOutcome([redacted])");
    let CredentialStageOutcome::Match(candidate) = outcome else {
        panic!("MD5 match required")
    };
    assert_eq!(
        format!("{candidate:?}"),
        "UnfinalizedCredentialMatch([redacted])"
    );
    let CredentialStage::NeedLogin(request) = begin(b"member@example.invalid", &password) else {
        panic!("login request required")
    };
    // Source length guard is definite false, even for unsupported hash formats.
    let CredentialStage::NeedEmail(request) =
        request.respond(CredentialLookup::Found(record(8, &[255; 33])))
    else {
        panic!("source-guard false may try email")
    };
    assert_eq!(format!("{request:?}"), "EmailLookupRequest([redacted])");
}

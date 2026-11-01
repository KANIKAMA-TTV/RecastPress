#![forbid(unsafe_code)]
use recastpress_wp::auth_cookie_verify::{
    CookieRejectionStage as Stage, CookieSessionExpiration as SessionExpiration,
    CookieVerificationLimits as VerifyLimits, CookieVerificationOutcome as Outcome,
    PreparedCookieSession as PreparedSession, PreparedCookieSessions as PreparedSessions,
    PreparedCookieVerificationUser as PreparedUser,
    wp_validate_auth_cookie_prepared_get_non_ajax_resolved_salt_bytes as check,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: u32,
    cookie_hex: String,
    scheme_hex: String,
    salt_hex: String,
    user: UserInput,
    sessions: Vec<SessionWire>,
    now: String,
    limits: LimitsWire,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum UserInput {
    Null(()),
    User(UserWire),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserWire {
    id: String,
    login_hex: String,
    hash_hex: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionWire {
    verifier: String,
    expiration: String,
    legacy: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitsWire {
    max_input_bytes: usize,
    max_sessions: usize,
    max_session_key_bytes: usize,
}
#[derive(Serialize)]
struct Report {
    format_version: u32,
    process_pid: u32,
    cases: Vec<ResultWire>,
}
#[derive(Serialize)]
struct ResultWire {
    id: u32,
    outcome: &'static str,
    stage: &'static str,
    prepared_user_id: Option<String>,
}
// Relation wire guard only; the library keeps its canonical expiry parser private.
fn canonical_wire(raw: &[u8]) -> Result<i64, ()> {
    let text = std::str::from_utf8(raw).map_err(|_| ())?;
    let value = text.parse::<i64>().map_err(|_| ())?;
    if value.to_string().as_bytes() != raw {
        return Err(());
    }
    Ok(value)
}

fn decode(value: &str) -> Result<Vec<u8>, ()> {
    if value.len() > 131072 || !value.len().is_multiple_of(2) {
        return Err(());
    }
    let digit = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|p| Ok(digit(p[0]).ok_or(())? * 16 + digit(p[1]).ok_or(())?))
        .collect()
}
fn run() -> Result<(), ()> {
    if std::env::args_os().len() != 1 {
        return Err(());
    }
    let mut raw = Vec::new();
    std::io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| ())?;
    if raw.len() > 8 * 1024 * 1024 {
        return Err(());
    }
    let input: Envelope = serde_json::from_slice(&raw).map_err(|_| ())?;
    if input.cases.is_empty() || input.cases.len() > 256 {
        return Err(());
    }
    let mut output = Vec::new();
    for (index, c) in input.cases.iter().enumerate() {
        if input.cases[..index].iter().any(|old| old.id == c.id)
            || (
                c.limits.max_input_bytes,
                c.limits.max_sessions,
                c.limits.max_session_key_bytes,
            ) != (65536, 64, 4096)
        {
            return Err(());
        }
        let cookie = decode(&c.cookie_hex)?;
        let scheme = decode(&c.scheme_hex)?;
        let salt = decode(&c.salt_hex)?;
        let user_bytes = match &c.user {
            UserInput::Null(_) => None,
            UserInput::User(u) => Some((
                canonical_wire(u.id.as_bytes()).map_err(|_| ())?,
                decode(&u.login_hex)?,
                decode(&u.hash_hex)?,
            )),
        };
        let user = match &user_bytes {
            Some((id, login, hash)) => Some(PreparedUser::new(*id, login, hash).map_err(|_| ())?),
            None => None,
        };
        let entries: Vec<_> = c
            .sessions
            .iter()
            .map(|s| {
                let expiry = canonical_wire(s.expiration.as_bytes()).map_err(|_| ())?;
                Ok(PreparedSession {
                    verifier: s.verifier.as_bytes(),
                    expiration: if s.legacy {
                        SessionExpiration::Legacy(expiry)
                    } else {
                        SessionExpiration::Record(expiry)
                    },
                })
            })
            .collect::<Result<_, ()>>()?;
        let owner = user_bytes.as_ref().map_or(1, |(id, _, _)| *id);
        let sessions = PreparedSessions::new(
            owner,
            &entries,
            c.limits.max_sessions,
            c.limits.max_session_key_bytes,
        )
        .map_err(|_| ())?;
        let now = canonical_wire(c.now.as_bytes()).map_err(|_| ())?;
        let outcome = check(
            &cookie,
            &scheme,
            user.as_ref(),
            &sessions,
            &salt,
            now,
            VerifyLimits::new(c.limits.max_input_bytes),
        )
        .map_err(|_| ())?;
        let (outcome, stage, prepared_user_id) = match outcome {
            Outcome::Match(proof) => (
                "match",
                "valid",
                Some(proof.prepared_record_id().to_string()),
            ),
            Outcome::NoMatch(stage) => (
                "no_match",
                match stage {
                    Stage::Malformed => "malformed",
                    Stage::Expired => "expired",
                    Stage::UnknownUser => "unknown_user",
                    Stage::BadMac => "bad_mac",
                    Stage::BadSession => "bad_session",
                },
                None,
            ),
        };
        output.push(ResultWire {
            id: c.id,
            outcome,
            stage,
            prepared_user_id,
        });
    }
    let mut wire = serde_json::to_vec(&Report {
        format_version: 1,
        process_pid: std::process::id(),
        cases: output,
    })
    .map_err(|_| ())?;
    wire.push(b'\n');
    if wire.len() > 1024 * 1024 {
        return Err(());
    }
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&wire)
        .and_then(|_| stdout.flush())
        .map_err(|_| ())
}
fn main() {
    if run().is_err() {
        let _ = std::io::stderr()
            .lock()
            .write_all(b"prepared cookie proof refused\n");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn schema_refuses_unknown_missing_duplicate_and_scalar_type_confusion() {
        let good = r#"{"cases":[{"id":1,"cookie_hex":"","scheme_hex":"","salt_hex":"","user":null,"sessions":[],"now":"0","limits":{"max_input_bytes":65536,"max_sessions":64,"max_session_key_bytes":4096}}]}"#;
        assert!(serde_json::from_str::<Envelope>(good).is_ok());
        for changed in [
            good.replace("\"id\":1", "\"id\":true"),
            good.replace("\"id\":1", "\"id\":1.0"),
            good.replace("\"id\":1", "\"id\":1,\"id\":1"),
            good.replace("\"user\":null,", ""),
            good.replace("\"now\":\"0\"", "\"now\":\"0\",\"PRIVATE_UNKNOWN\":true"),
            good.replace("\"cases\":", "\"PRIVATE_UNKNOWN\":[],\"cases\":"),
        ] {
            assert!(serde_json::from_str::<Envelope>(&changed).is_err());
        }
        let duplicate_user = good.replace(
            "\"user\":null",
            "\"user\":{\"id\":\"1\",\"id\":\"1\",\"login_hex\":\"\",\"hash_hex\":\"\"}",
        );
        assert!(serde_json::from_str::<Envelope>(&duplicate_user).is_err());
    }
    #[test]
    fn byte_codec_rejects_bad_hex_without_replacing_arbitrary_bytes() {
        assert_eq!(decode("00Ff80").unwrap(), b"\0\xff\x80");
        for bad in ["0", "gg", "PRIVATE", "00 0"] {
            assert!(decode(bad).is_err());
        }
    }
}

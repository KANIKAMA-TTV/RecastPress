//! Finite public-dummy record relations, not a credential/authentication endpoint.
use recastpress_wp::{
    CredentialLookup, CredentialLookupKind, CredentialPhase, CredentialStage,
    CredentialStageOutcome, CredentialUnresolvedReason, HostMaxBcryptCost, HostMaxPhpassCost,
    HostPasswordWorkLimits, PreparedLoginInput, RehashIntent,
};
use serde_json::{Value, json};
use std::io::{self, Read};

struct Record {
    key: Vec<u8>,
    hash: Vec<u8>,
    tag: String,
}
impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SECRET_RECORD_HASH")
    }
}
fn fields(value: &Value, expected: &[&str]) -> Result<(), &'static str> {
    let map = value.as_object().ok_or("object required")?;
    if map.len() != expected.len() || expected.iter().any(|key| !map.contains_key(*key)) {
        return Err("fields differ");
    }
    Ok(())
}
fn unhex(value: &Value) -> Result<Vec<u8>, &'static str> {
    let text = value.as_str().ok_or("byte wire required")?;
    if text.len() > 16384 || !text.len().is_multiple_of(2) {
        return Err("byte bound");
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |b| match b {
                b'0'..=b'9' => Ok(b - b'0'),
                b'a'..=b'f' => Ok(b - b'a' + 10),
                _ => Err("noncanonical hex"),
            };
            Ok(digit(pair[0])? * 16 + digit(pair[1])?)
        })
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        use std::fmt::Write;
        write!(out, "{byte:02x}").expect("string writer");
    }
    out
}
fn record(value: &Value) -> Result<Option<Record>, &'static str> {
    if value.is_null() {
        return Ok(None);
    }
    fields(value, &["key_hex", "hash_hex", "tag"])?;
    let tag = value["tag"].as_str().ok_or("tag required")?;
    if !matches!(tag, "login-record" | "email-record") {
        return Err("public tag required");
    }
    Ok(Some(Record {
        key: unhex(&value["key_hex"])?,
        hash: unhex(&value["hash_hex"])?,
        tag: tag.to_owned(),
    }))
}
fn phase(value: CredentialPhase) -> &'static str {
    match value {
        CredentialPhase::Input => "input",
        CredentialPhase::Login => "login",
        CredentialPhase::Email => "email",
    }
}
fn kind(value: CredentialLookupKind) -> &'static str {
    match value {
        CredentialLookupKind::Login => "login",
        CredentialLookupKind::Email => "email",
    }
}
fn reason(value: CredentialUnresolvedReason) -> &'static str {
    match value {
        CredentialUnresolvedReason::Lookup => "lookup",
        CredentialUnresolvedReason::Preprocessing => "preprocessing",
        CredentialUnresolvedReason::UnsupportedHash => "unsupported_hash",
        CredentialUnresolvedReason::BcryptCostLimit => "bcrypt_cost_limit",
        CredentialUnresolvedReason::PortableCostLimit => "portable_cost_limit",
        CredentialUnresolvedReason::ProviderFailure => "provider_failure",
        CredentialUnresolvedReason::RehashMetadata => "rehash_metadata",
        CredentialUnresolvedReason::EmailProfile => "email_profile",
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut wire = Vec::new();
    io::stdin().take(1048577).read_to_end(&mut wire)?;
    if wire.len() > 1048576 {
        return Err("input bound".into());
    }
    let request: Value = serde_json::from_slice(&wire).map_err(|_| "invalid request")?;
    if serde_json::to_vec(&request)? != wire {
        return Err("noncanonical request".into());
    }
    fields(&request, &["items"])?;
    let items = request["items"].as_array().ok_or("items required")?;
    if items.len() > 128 {
        return Err("batch bound".into());
    }
    let mut results = Vec::new();
    for item in items {
        fields(
            item,
            &[
                "username_hex",
                "password_hex",
                "locale_hex",
                "login",
                "email",
                "bcrypt_maximum",
                "portable_maximum",
                "lookup_error",
            ],
        )?;
        let raw = unhex(&item["username_hex"])?;
        let password = unhex(&item["password_hex"])?;
        let locale = unhex(&item["locale_hex"])?;
        let mut login = record(&item["login"])?;
        let mut email = record(&item["email"])?;
        let lookup_error = item["lookup_error"]
            .as_str()
            .ok_or("lookup error mode required")?;
        if !matches!(lookup_error, "none" | "login" | "email") {
            return Err("lookup error mode differs".into());
        }
        let limits = HostPasswordWorkLimits::new(
            HostMaxBcryptCost::new(u8::try_from(
                item["bcrypt_maximum"]
                    .as_u64()
                    .ok_or("bcrypt maximum required")?,
            )?)?,
            HostMaxPhpassCost::new(u8::try_from(
                item["portable_maximum"]
                    .as_u64()
                    .ok_or("portable maximum required")?,
            )?)?,
        );
        let prepared = PreparedLoginInput::new(&raw, &password, &locale, 65536)
            .map_err(|_| "preparation refused")?;
        let username_empty = prepared.username_php_empty();
        let password_empty = prepared.password_php_empty();
        if format!("{prepared:?}").contains("SECRET_") {
            return Err("debug privacy failed".into());
        }
        let mut stage = CredentialStage::begin(prepared, limits, |row: &Record| &row.hash);
        let mut lookups = Vec::new();
        let outcome = loop {
            if format!("{stage:?}") != "CredentialStage([redacted])" {
                return Err("stage debug privacy failed".into());
            }
            if lookups.len() > 2 {
                return Err("stage request bound".into());
            }
            stage = match stage {
                CredentialStage::NeedLogin(request) => {
                    if format!("{request:?}") != "LoginLookupRequest([redacted])" {
                        return Err("request debug privacy failed".into());
                    }
                    let key = request.query_key();
                    lookups.push(json!({"kind":"login","key_hex":hex(key)}));
                    let reply = if lookup_error == "login" {
                        CredentialLookup::LookupUnresolved
                    } else if login.as_ref().is_some_and(|row| row.key == key) {
                        CredentialLookup::Found(login.take().ok_or("record disappeared")?)
                    } else {
                        CredentialLookup::NotFound
                    };
                    request.respond(reply)
                }
                CredentialStage::NeedEmail(request) => {
                    if format!("{request:?}") != "EmailLookupRequest([redacted])" {
                        return Err("request debug privacy failed".into());
                    }
                    let key = request.query_key();
                    lookups.push(json!({"kind":"email","key_hex":hex(key)}));
                    let reply = if lookup_error == "email" {
                        CredentialLookup::LookupUnresolved
                    } else if email.as_ref().is_some_and(|row| row.key == key) {
                        CredentialLookup::Found(email.take().ok_or("record disappeared")?)
                    } else {
                        CredentialLookup::NotFound
                    };
                    request.respond(reply)
                }
                CredentialStage::Complete(outcome) => break outcome,
            };
        };
        if format!("{outcome:?}") != "CredentialStageOutcome([redacted])" {
            return Err("outcome debug privacy failed".into());
        }
        let mut result = json!({"outcome":"no_match","record_tag":null,"lookups":lookups,"rehash_hint":null,
            "unresolved_reason":null,"matched_via":null,"unresolved_phase":null,
            "username_php_empty":username_empty,"password_php_empty":password_empty});
        match outcome {
            CredentialStageOutcome::NoCredentialMatch => {}
            CredentialStageOutcome::Match(value) => {
                if format!("{value:?}") != "UnfinalizedCredentialMatch([redacted])" {
                    return Err("match debug privacy failed".into());
                }
                result["outcome"] = json!("user");
                result["record_tag"] = json!(value.record().tag);
                result["matched_via"] = json!(kind(value.matched_via()));
                result["rehash_hint"] = json!(value.rehash_intent() == RehashIntent::Suggested);
            }
            CredentialStageOutcome::Unresolved { at, reason: why } => {
                result["outcome"] = json!("unresolved");
                result["unresolved_reason"] = json!(reason(why));
                result["unresolved_phase"] = json!(phase(at));
            }
        }
        results.push(result);
    }
    let output = serde_json::to_vec(&json!({"process_pid":std::process::id(),"results":results}))?;
    if output.len() > 131072 {
        return Err("output bound".into());
    }
    println!("{}", std::str::from_utf8(&output)?);
    Ok(())
}
fn main() {
    if run().is_err() {
        eprintln!("Credential stage probe rejected");
        std::process::exit(2);
    }
}

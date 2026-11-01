//! Finite public-dummy legacy verification, outside the compatibility registry.
use recastpress_wp::{
    HostMaxBcryptCost, HostMaxPhpassCost, HostPasswordWorkLimits, LegacyPasswordVerifyError,
    PasswordDispatchError, wp_check_password_legacy_md5_bytes,
    wp_check_password_phpass_portable_bytes, wp_check_password_raw_empty_filters_bytes,
};
use serde_json::{Value, json};
use std::io::{self, Read};

fn fields(value: &Value, expected: &[&str]) -> Result<(), &'static str> {
    let object = value.as_object().ok_or("object required")?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("unknown or missing field");
    }
    Ok(())
}
fn unhex(value: &Value) -> Result<Vec<u8>, &'static str> {
    let text = value.as_str().ok_or("byte wire required")?;
    if text.len() > 16384 || !text.len().is_multiple_of(2) {
        return Err("hex outside probe domain");
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                _ => Err("noncanonical hex"),
            };
            Ok(digit(pair[0])? * 16 + digit(pair[1])?)
        })
        .collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut wire = Vec::new();
    io::stdin().take(1048577).read_to_end(&mut wire)?;
    if wire.len() > 1048576 {
        return Err("probe input bound".into());
    }
    let request: Value = serde_json::from_slice(&wire)?;
    if serde_json::to_vec(&request)? != wire {
        return Err("noncanonical request".into());
    }
    fields(&request, &["items"])?;
    let items = request["items"].as_array().ok_or("items required")?;
    if items.len() > 128 {
        return Err("probe count bound".into());
    }
    let mut results = Vec::new();
    for item in items {
        let mode = item["mode"].as_str().ok_or("mode required")?;
        fields(
            item,
            if mode == "md5" {
                &["mode", "password_hex", "hash_hex"]
            } else if mode == "combined" {
                &[
                    "mode",
                    "password_hex",
                    "hash_hex",
                    "bcrypt_maximum",
                    "portable_maximum",
                ]
            } else {
                &["mode", "password_hex", "hash_hex", "maximum"]
            },
        )?;
        let password = unhex(&item["password_hex"])?;
        let hash = unhex(&item["hash_hex"])?;
        let result = if mode == "combined" {
            // Both typed allowances are explicit even when this particular
            // branch never invokes either expensive provider.
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
            match wp_check_password_raw_empty_filters_bytes(&password, &hash, limits) {
                Ok(value) => json!({"verified":value}),
                Err(PasswordDispatchError::UnsupportedHash) => json!({"error":"unsupported_hash"}),
                Err(PasswordDispatchError::BcryptCostLimit) => json!({"error":"bcrypt_cost_limit"}),
                Err(PasswordDispatchError::PortableCostLimit) => {
                    json!({"error":"portable_cost_limit"})
                }
                Err(PasswordDispatchError::ProviderFailure) => {
                    return Err("password verification provider failed".into());
                }
            }
        } else {
            let result = match mode {
                "md5" => wp_check_password_legacy_md5_bytes(&password, &hash),
                "phpass" => wp_check_password_phpass_portable_bytes(
                    &password,
                    &hash,
                    HostMaxPhpassCost::new(u8::try_from(
                        item["maximum"].as_u64().ok_or("maximum required")?,
                    )?)?,
                ),
                _ => return Err("unsupported probe mode".into()),
            };
            match result {
                Ok(value) => json!({"verified":value}),
                Err(LegacyPasswordVerifyError::UnsupportedHash) => {
                    json!({"error":"unsupported_hash"})
                }
                Err(LegacyPasswordVerifyError::CostLimit) => json!({"error":"cost_limit"}),
            }
        };
        results.push(result);
    }
    let output = serde_json::to_vec(&json!({"process_pid":std::process::id(),"results":results}))?;
    if output.len() > 65536 {
        return Err("probe output bound".into());
    }
    println!("{}", std::str::from_utf8(&output)?);
    Ok(())
}

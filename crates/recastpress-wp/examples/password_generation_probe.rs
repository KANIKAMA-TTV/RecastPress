//! Finite public-dummy cross-verification probe, outside the primitive registry.
use recastpress_wp::{
    HostMaxBcryptCost, WpPasswordHashError, wp_check_password_wp_bcrypt_bytes,
    wp_hash_password_default_bcrypt_bytes,
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
fn unhex(text: &str) -> Result<Vec<u8>, &'static str> {
    if text.len() > 8194 || !text.len().is_multiple_of(2) {
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
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut wire = Vec::new();
    io::stdin().take(65537).read_to_end(&mut wire)?;
    if wire.len() > 65536 {
        return Err("probe input exceeds bound".into());
    }
    let request: Value = serde_json::from_slice(&wire)?;
    // The dedicated harness sends compact sorted-key ASCII JSON only. This
    // round trip also rejects duplicate keys before any generation/provider call.
    if serde_json::to_vec(&request)? != wire {
        return Err("noncanonical request".into());
    }
    fields(&request, &["operation", "items"])?;
    let operation = request["operation"].as_str().ok_or("operation required")?;
    let items = request["items"].as_array().ok_or("items required")?;
    if items.len() > 64 {
        return Err("probe item count exceeds bound".into());
    }
    let mut results = Vec::new();
    for item in items {
        fields(
            item,
            if operation == "generate" {
                &["password_hex", "maximum"]
            } else {
                &["password_hex", "hash_hex", "maximum"]
            },
        )?;
        let password = unhex(item["password_hex"].as_str().ok_or("password required")?)?;
        let maximum = HostMaxBcryptCost::new(u8::try_from(
            item["maximum"].as_u64().ok_or("maximum required")?,
        )?)?;
        let result = match operation {
            "generate" => match wp_hash_password_default_bcrypt_bytes(&password, maximum) {
                Ok(value) => json!({"hash_hex":hex(value.as_wp_storage_bytes())}),
                Err(WpPasswordHashError::CostLimit) => json!({"error":"cost_limit"}),
                Err(_) => return Err("password generation provider failed".into()),
            },
            "verify" => {
                let hash = unhex(item["hash_hex"].as_str().ok_or("hash is required")?)?;
                json!({"verified":wp_check_password_wp_bcrypt_bytes(&password,&hash,maximum)?})
            }
            _ => return Err("unsupported probe operation".into()),
        };
        results.push(result);
    }
    let output = serde_json::to_vec(&json!({"process_pid":std::process::id(),"results":results}))?;
    if output.len() > 65536 {
        return Err("probe output exceeds bound".into());
    }
    println!("{}", std::str::from_utf8(&output)?);
    Ok(())
}

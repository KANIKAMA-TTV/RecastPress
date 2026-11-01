//! Selected parse_str grammar for explicit callers, not an HTTP request parser.

use crate::{
    CompatibilityError,
    strings::decode_form_bytes,
    value::{PhpArray, PhpKey, PhpValue},
};

/// PHP's default ampersand separator, at most 1,000 nonempty components, and
/// byte-string input only. Array notation, NUL in input/decoded keys, configurable
/// separators/limits and their diagnostics are explicitly unsupported.
pub fn parse_flat_query(input: &[u8]) -> Result<PhpArray, CompatibilityError> {
    parse_query(input, false)
}

/// Adds balanced bracket paths, empty-bracket append and at most 64 bracket
/// levels. Whitespace inside bracket components, malformed paths, NUL keys,
/// append overflow and PHP parser diagnostics remain explicitly unsupported.
pub fn parse_query_bytes(input: &[u8]) -> Result<PhpArray, CompatibilityError> {
    parse_query(input, true)
}

fn parse_query(input: &[u8], arrays: bool) -> Result<PhpArray, CompatibilityError> {
    if input.contains(&0) {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let mut result = PhpArray::new();
    let mut count = 0;
    for part in input
        .split(|&byte| byte == b'&')
        .filter(|part| !part.is_empty())
    {
        count += 1;
        if count > 1000 {
            return Err(CompatibilityError::UnsupportedValue);
        }
        let split = part
            .iter()
            .position(|&byte| byte == b'=')
            .unwrap_or(part.len());
        let key = decode_form_bytes(&part[..split]);
        let Some(path) = key_path(&key, arrays)? else {
            continue;
        };
        let value = if split == part.len() {
            Vec::new()
        } else {
            decode_form_bytes(&part[split + 1..])
        };
        assign(&mut result, &path, PhpValue::Bytes(value))?;
    }
    Ok(result)
}

fn key_path(key: &[u8], arrays: bool) -> Result<Option<Vec<Option<PhpKey>>>, CompatibilityError> {
    if key.contains(&0) {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let start = key
        .iter()
        .position(|&byte| byte != b' ')
        .unwrap_or(key.len());
    let key = &key[start..];
    let first = key
        .iter()
        .position(|&byte| byte == b'[')
        .unwrap_or(key.len());
    if key[..first].contains(&b']') {
        return Err(CompatibilityError::UnsupportedValue);
    }
    if first == 0 {
        return if key.is_empty() {
            Ok(None)
        } else {
            Err(CompatibilityError::UnsupportedValue)
        };
    }
    let base: Vec<_> = key[..first]
        .iter()
        .map(|&byte| {
            if matches!(byte, b'.' | b' ') {
                b'_'
            } else {
                byte
            }
        })
        .collect();
    let mut path = vec![Some(PhpKey::from_bytes(&base))];
    let mut position = first;
    while position < key.len() {
        if !arrays || key[position] != b'[' {
            return Err(CompatibilityError::UnsupportedValue);
        }
        let end = key[position + 1..]
            .iter()
            .position(|&byte| byte == b']')
            .map(|offset| position + 1 + offset)
            .ok_or(CompatibilityError::UnsupportedValue)?;
        let part = &key[position + 1..end];
        if part
            .iter()
            .any(|&byte| byte == b'[' || byte.is_ascii_whitespace())
        {
            return Err(CompatibilityError::UnsupportedValue);
        }
        path.push(if part.is_empty() {
            None
        } else {
            Some(PhpKey::from_bytes(part))
        });
        if path.len() > 65 {
            return Err(CompatibilityError::RecursionLimit);
        }
        position = end + 1;
    }
    Ok(Some(path))
}

fn assign(
    array: &mut PhpArray,
    path: &[Option<PhpKey>],
    value: PhpValue,
) -> Result<(), CompatibilityError> {
    let key = match &path[0] {
        Some(key) => key.clone(),
        None => {
            let maximum = array
                .iter()
                .filter_map(|(key, _)| match key {
                    PhpKey::Integer(value) => Some(*value),
                    _ => None,
                })
                .max();
            PhpKey::Integer(match maximum {
                None => 0,
                Some(maximum) => maximum
                    .checked_add(1)
                    .ok_or(CompatibilityError::UnsupportedValue)?,
            })
        }
    };
    if path.len() == 1 {
        array.insert(key, value);
        return Ok(());
    }
    if !matches!(array.get(&key), Some(PhpValue::Array(_))) {
        array.insert(key.clone(), PhpValue::Array(PhpArray::new()));
    }
    let Some(PhpValue::Array(child)) = array.get_mut(&key) else {
        unreachable!("initialized canonical array key")
    };
    assign(child, &path[1..], value)
}

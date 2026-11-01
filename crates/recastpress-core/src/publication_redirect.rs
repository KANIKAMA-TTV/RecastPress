use super::{MAX_TARGET_BYTES, PrepareError};
use std::sync::Arc;

/// Explicit statuses, separate from the legacy text/raster status domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectStatus {
    MovedPermanently,
    Found,
    SeeOther,
    TemporaryRedirect,
    PermanentRedirect,
}

impl RedirectStatus {
    pub fn code(self) -> u16 {
        match self {
            Self::MovedPermanently => 301,
            Self::Found => 302,
            Self::SeeOther => 303,
            Self::TemporaryRedirect => 307,
            Self::PermanentRedirect => 308,
        }
    }
}

impl TryFrom<u16> for RedirectStatus {
    type Error = PrepareError;
    fn try_from(code: u16) -> Result<Self, Self::Error> {
        match code {
            301 => Ok(Self::MovedPermanently),
            302 => Ok(Self::Found),
            303 => Ok(Self::SeeOther),
            307 => Ok(Self::TemporaryRedirect),
            308 => Ok(Self::PermanentRedirect),
            _ => Err(PrepareError::InvalidStatus),
        }
    }
}

/// Immutable root-relative ASCII Location subset, not a general URL parser.
/// Input is preserved without trim/decode/escape/normalization. Percent pairs
/// must be complete; encoded controls, space, slash/backslash are excluded by
/// host policy. RFC3986 pchar, slash and query '?' are otherwise supported.
/// This does not prove arbitrary downstream normalization or application
/// behavior; fragment inheritance is left to HTTP user agents. The length bound
/// is not RSS or shared-budget accounting, including this Arc's metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginFormLocation(Arc<str>);

impl OriginFormLocation {
    pub fn new(value: String) -> Result<Self, PrepareError> {
        if !valid(&value) {
            return Err(PrepareError::InvalidLocation);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn valid(value: &str) -> bool {
    if value.len() > MAX_TARGET_BYTES || !value.starts_with('/') || value.starts_with("//") {
        return false;
    }
    let bytes = value.as_bytes();
    let mut offset = 0;
    while let Some(&byte) = bytes.get(offset) {
        if byte == b'%' {
            let Some(high) = bytes.get(offset + 1).copied().and_then(nibble) else {
                return false;
            };
            let Some(low) = bytes.get(offset + 2).copied().and_then(nibble) else {
                return false;
            };
            let decoded = (high << 4) | low;
            if decoded <= 0x20 || matches!(decoded, 0x7f | b'/' | b'\\') {
                return false;
            }
            offset += 3;
            continue;
        }
        if !(byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'/'
                    | b'?'
            ))
        {
            return false;
        }
        offset += 1;
    }
    true
}

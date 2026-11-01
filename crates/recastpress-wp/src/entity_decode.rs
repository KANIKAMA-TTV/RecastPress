//! Byte-value subset of WordPress's two-pass numeric entity decoder.

use std::fmt;

const INPUT_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityDecodeError {
    /// A complete recognized reference, including one generated between passes,
    /// has a value outside 0..=255. PHP 8.5 diagnostics are not reproduced.
    UnsupportedEntityValue,
    /// Prototype whole-input bound, separate from the final output allowance.
    InputLimit,
    /// Final output byte cap, length overflow or intermediate/output allocation failure.
    OutputLimit,
}

impl fmt::Display for EntityDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedEntityValue => {
                "entity decoder recognizes only values from 0 through 255"
            }
            Self::InputLimit => "entity decoder input exceeds the host input byte bound",
            Self::OutputLimit => "entity decoder exceeds the host output byte or allocation bound",
        })
    }
}

impl std::error::Error for EntityDecodeError {}

#[derive(Clone, Copy)]
enum Pass {
    Decimal,
    Hex,
}

fn reference(input: &[u8], pass: Pass) -> Result<Option<(usize, u8)>, EntityDecodeError> {
    if !input.starts_with(b"&#") {
        return Ok(None);
    }
    let (start, radix) = match pass {
        Pass::Decimal => (2, 10),
        Pass::Hex if matches!(input.get(2), Some(b'x' | b'X')) => (3, 16),
        Pass::Hex => return Ok(None),
    };
    let mut end = start;
    let mut value = 0u16;
    while let Some(&byte) = input.get(end) {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' if radix == 16 => byte - b'a' + 10,
            b'A'..=b'F' if radix == 16 => byte - b'A' + 10,
            _ => break,
        };
        // The sentinel cannot return to the byte domain. Multiplication stays
        // below 4112 even for arbitrarily long zero/overflow digit strings.
        value = (value * radix + u16::from(digit)).min(256);
        end += 1;
    }
    if end == start || input.get(end) != Some(&b';') {
        // Even enormous numbers are ordinary bytes when the regex would not
        // match. Do not refuse an unfinished/malformed reference prematurely.
        return Ok(None);
    }
    let byte = u8::try_from(value).map_err(|_| EntityDecodeError::UnsupportedEntityValue)?;
    Ok(Some((end + 1, byte)))
}

fn scan(
    input: &[u8],
    pass: Pass,
    mut output: Option<&mut Vec<u8>>,
) -> Result<usize, EntityDecodeError> {
    let mut position = 0;
    let mut length = 0usize;
    while position < input.len() {
        let (consumed, decoded) = match reference(&input[position..], pass)? {
            Some((consumed, byte)) => (consumed, byte),
            None => (1, input[position]),
        };
        length = length
            .checked_add(1)
            .ok_or(EntityDecodeError::OutputLimit)?;
        if let Some(bytes) = &mut output {
            bytes.push(decoded);
        }
        position += consumed;
    }
    Ok(length)
}

fn allocate(length: usize) -> Result<Vec<u8>, EntityDecodeError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| EntityDecodeError::OutputLimit)?;
    Ok(bytes)
}

/// WordPress 7.1.3 `kses.php:2376` byte-input subset using its decimal-then-hex
/// order, with a single nonrecursive pass of each kind. Only complete references
/// whose recognized value is 0..=255 are supported. The hex pass also validates
/// references created by decimal decoding. Names and malformed forms are retained;
/// decoded values are raw bytes, including NUL and invalid UTF-8, not Unicode.
///
/// The whole-input limit is 1MiB, checked before parsing/allocation. A measured
/// decimal intermediate Vec (at most input length) is allocated before hex-domain
/// validation and the final output cap. This intermediate, input and allocator
/// overhead are unpooled and outside that final-byte cap/RSS accounting. The hex
/// pass validates/measures its complete input before cap checking and final output
/// reservation, so compressed output is not rejected by its pre-decoding size.
///
/// InputLimit and OutputLimit are host policies, not PHP or product capacity
/// settings. PHP scalar casts, out-of-range chr warnings/modulo/float behavior,
/// PCRE failures, HTML sanitization, escaping, charset selection and output safety
/// are unmodeled. No shared budget, public HTTP or raw DB path is connected.
pub fn wp_kses_decode_entities_byte_values(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, EntityDecodeError> {
    if input.len() > INPUT_LIMIT {
        return Err(EntityDecodeError::InputLimit);
    }
    let intermediate_length = scan(input, Pass::Decimal, None)?;
    let mut intermediate = allocate(intermediate_length)?;
    scan(input, Pass::Decimal, Some(&mut intermediate))?;
    let output_length = scan(&intermediate, Pass::Hex, None)?;
    if output_length > max_output_bytes {
        return Err(EntityDecodeError::OutputLimit);
    }
    let mut output = allocate(output_length)?;
    scan(&intermediate, Pass::Hex, Some(&mut output))?;
    Ok(output)
}

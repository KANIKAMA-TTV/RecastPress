//! Narrow byte-pair replacement used by WordPress KSES.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KsesSlashesError {
    /// Final output byte cap, checked length overflow or output allocation failure.
    OutputLimit,
}

impl fmt::Display for KsesSlashesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KSES slash removal exceeds the host output byte or allocation bound")
    }
}

impl std::error::Error for KsesSlashesError {}

fn scan(input: &[u8], mut output: Option<&mut Vec<u8>>) -> Result<usize, KsesSlashesError> {
    let mut position = 0;
    let mut length = 0usize;
    while position < input.len() {
        let byte = if input[position] == b'\\' && input.get(position + 1) == Some(&b'"') {
            position += 2;
            b'"'
        } else {
            let byte = input[position];
            position += 1;
            byte
        };
        length = length.checked_add(1).ok_or(KsesSlashesError::OutputLimit)?;
        if let Some(bytes) = &mut output {
            bytes.push(byte);
        }
    }
    Ok(length)
}

/// WordPress 7.1.3 `kses.php:2044` for byte-string input: replace each literal
/// backslash + double-quote pair with a double quote in one nonrecursive pass.
/// A run of n backslashes before a quote loses only its final backslash.
/// Other escapes, trailing backslashes, entities, NUL and invalid UTF-8 remain.
/// This is deliberately different from PHP stripslashes and WordPress wp_unslash.
///
/// The same traversal measures and emits complete output, with the final byte
/// cap checked before reservation. No intermediate Vec or input bound is used.
/// OutputLimit includes checked overflow and allocation failure; it is a host
/// policy, not PHP behavior, a product capacity setting or shared RAM/RSS budget.
/// Non-byte coercions/diagnostics, PCRE failures, recursive escaping, HTML
/// sanitization/output safety and public HTTP/DB integration are unmodeled.
pub fn wp_kses_stripslashes_bytes(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, KsesSlashesError> {
    let length = scan(input, None)?;
    if length > max_output_bytes {
        return Err(KsesSlashesError::OutputLimit);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| KsesSlashesError::OutputLimit)?;
    scan(input, Some(&mut output))?;
    Ok(output)
}

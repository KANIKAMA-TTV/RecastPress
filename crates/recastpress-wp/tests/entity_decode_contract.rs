use recastpress_wp::{EntityDecodeError, wp_kses_decode_entities_byte_values as decode};

#[test]
fn complete_decimal_and_hex_references_decode_every_byte_including_zero() {
    for byte in 0..=255u8 {
        let input = format!("&#{byte};&#x{byte:x};&#X{byte:X};");
        assert_eq!(decode(input.as_bytes(), 3), Ok(vec![byte; 3]));
    }
}

#[test]
fn decimal_generation_feeds_one_hex_pass_without_repeating_either_pass() {
    for (input, expected) in [
        (b"&#38;#x41;".as_slice(), b"A".as_slice()),
        (b"&#38;&#35;x41;", b"A"),
        (b"&#38;#&#120;41;", b"A"),
        (b"&#38;&#35;&#120;&#52;&#49;&#59;", b"A"),
        (b"&#38;#x41&#59;", b"A"),
        (b"&#38;#x&#0;41;", b"&#x\0\x34\x31;"),
        (b"&#38;#x&#255;41;", b"&#x\xff41;"),
        (b"&#x1&#50;;", b"\x12"),
        (b"&#38;#65;", b"&#65;"),
        (b"&#x26;#65;", b"&#65;"),
        (b"&#x26;#x41;", b"&#x41;"),
        (b"&#x26;&#35;&#120;&#52;&#49;&#59;", b"&#x41;"),
        (b"&amp;#x41;", b"&amp;#x41;"),
    ] {
        assert_eq!(decode(input, expected.len()), Ok(expected.to_vec()));
    }
}

#[test]
fn recognized_out_of_byte_values_are_rejected_even_when_generated_between_passes() {
    for input in [
        b"&#256;".as_slice(),
        b"&#000256;",
        b"&#x100;",
        b"&#X000D800;",
        b"&#9223372036854775808;",
        b"&#xFFFFFFFFFFFFFFFF;",
        b"&#38;#x100;",
        b"&#38;&#35;x100;",
        b"&#38;&#35;&#120;&#49;&#48;&#48;&#59;",
        b"&#x10&#48;;",
    ] {
        assert_eq!(
            decode(input, 0),
            Err(EntityDecodeError::UnsupportedEntityValue),
            "{input:?}"
        );
    }
    let mut huge = b"&#".to_vec();
    huge.extend_from_slice(&vec![b'9'; 16384]);
    huge.push(b';');
    assert_eq!(
        decode(&huge, 0),
        Err(EntityDecodeError::UnsupportedEntityValue)
    );
    huge.insert(huge.len() - 1, b'x');
    assert_eq!(decode(&huge, huge.len()), Ok(huge));
}

#[test]
fn final_cap_applies_after_both_shrinking_passes_and_leading_zeros_never_overflow() {
    assert_eq!(decode(b"", 0), Ok(Vec::new()));
    assert_eq!(decode(b"&#38;#x41;", 1), Ok(b"A".to_vec()));
    assert_eq!(
        decode(b"&#38;#x41;", 0),
        Err(EntityDecodeError::OutputLimit)
    );
    for prefix in [b"&#".as_slice(), b"&#x"] {
        let mut input = prefix.to_vec();
        input.extend_from_slice(&vec![b'0'; 16384]);
        input.extend_from_slice(if prefix == b"&#" { b"255;" } else { b"fF;" });
        assert_eq!(decode(&input, 1), Ok(vec![255]));
        assert_eq!(decode(&input, 0), Err(EntityDecodeError::OutputLimit));
    }
}

#[test]
fn named_unfinished_and_raw_binary_input_are_preserved_without_mutation() {
    let input = b"\xff\0&copy;&#-1;&#x-1;&#65&#xFF".to_vec();
    let original = input.clone();
    assert_eq!(decode(&input, input.len()), Ok(input.clone()));
    assert_eq!(input, original);
    assert_eq!(decode(b"&#0;&#128;", 2), Ok(vec![0, 128]));
}

#[test]
fn whole_input_host_bound_is_distinct_from_the_final_output_cap() {
    let input = vec![b'x'; 1024 * 1024];
    assert_eq!(decode(&input, input.len()), Ok(input.clone()));
    assert_eq!(
        decode(&input, input.len() - 1),
        Err(EntityDecodeError::OutputLimit)
    );
    let oversized = vec![b'x'; 1024 * 1024 + 1];
    assert_eq!(
        decode(&oversized, usize::MAX),
        Err(EntityDecodeError::InputLimit)
    );
}

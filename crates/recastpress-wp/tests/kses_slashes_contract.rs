use recastpress_wp::{KsesSlashesError, wp_kses_stripslashes_bytes as strip};

#[test]
fn only_literal_slash_quote_pairs_are_removed_once_without_recursive_run_processing() {
    for (input, expected) in [
        (b"\\\"".as_slice(), b"\"".as_slice()),
        (b"\\\\\"", b"\\\""),
        (b"\\\\\\\"", b"\\\\\""),
        (b"\\\"\\\"", b"\"\""),
        (b"\\'\\0\\x\\n\\r\\t\\", b"\\'\\0\\x\\n\\r\\t\\"),
    ] {
        assert_eq!(strip(input, expected.len()), Ok(expected.to_vec()));
    }
}

#[test]
fn all_bytes_are_preserved_except_the_exact_two_byte_pair() {
    for byte in 0..=255u8 {
        let input = [byte, b'\\', b'"', byte];
        assert_eq!(strip(&input, 3), Ok(vec![byte, b'"', byte]));
        let input = [b'\\', byte];
        let expected = if byte == b'"' {
            vec![b'"']
        } else {
            input.to_vec()
        };
        assert_eq!(strip(&input, expected.len()), Ok(expected));
    }
}

#[test]
fn final_cap_counts_actual_shrinking_and_long_runs_without_intermediate_allocation() {
    assert_eq!(strip(b"", 0), Ok(Vec::new()));
    assert_eq!(strip(b"\\\"", 1), Ok(b"\"".to_vec()));
    assert_eq!(strip(b"\\\"", 0), Err(KsesSlashesError::OutputLimit));
    let repeated = b"\\\"".repeat(16384);
    assert_eq!(strip(&repeated, 16384), Ok(vec![b'"'; 16384]));
    assert_eq!(strip(&repeated, 16383), Err(KsesSlashesError::OutputLimit));
    let mut run = vec![b'\\'; 65536];
    run.push(b'"');
    let mut expected = vec![b'\\'; 65535];
    expected.push(b'"');
    assert_eq!(strip(&run, expected.len()), Ok(expected.clone()));
    assert_eq!(
        strip(&run, expected.len() - 1),
        Err(KsesSlashesError::OutputLimit)
    );
}

#[test]
fn raw_binary_and_entity_text_are_not_decoded_and_input_is_immutable() {
    let input = b"\xff\0&quote;\\\"&#34;\\0\\'\\".to_vec();
    let original = input.clone();
    let expected = b"\xff\0&quote;\"&#34;\\0\\'\\";
    assert_eq!(strip(&input, expected.len()), Ok(expected.to_vec()));
    assert_eq!(input, original);
}

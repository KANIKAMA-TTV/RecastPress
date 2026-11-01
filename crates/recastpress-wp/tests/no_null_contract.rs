use recastpress_wp::{NoNullError, SlashZeroMode, wp_kses_no_null_bytes as clean};

#[test]
fn exact_control_set_is_removed_for_all_bytes_while_tab_lf_cr_del_and_high_bytes_survive() {
    for byte in 0..=255u8 {
        let expected = if matches!(byte, 0..=8 | 11 | 12 | 14..=31) {
            Vec::new()
        } else {
            vec![byte]
        };
        for mode in [SlashZeroMode::Remove, SlashZeroMode::Keep] {
            assert_eq!(clean(&[byte], mode, expected.len()), Ok(expected.clone()));
        }
    }
    assert_eq!(
        clean(b"\t\n\r\x7f\x80\xff", SlashZeroMode::Remove, 6),
        Ok(b"\t\n\r\x7f\x80\xff".to_vec())
    );
}

#[test]
fn control_filtering_creates_slash_zero_runs_before_the_optional_removal_pass() {
    for (input, removed, kept) in [
        (b"\\\0\x1f00".as_slice(), b"".as_slice(), b"\\00".as_slice()),
        (b"\\\x1f\\0", b"", b"\\\\0"),
        (b"x\\\0\x1f00y", b"xy", b"x\\00y"),
        (b"\\0\n0", b"\n0", b"\\0\n0"),
        (b"\\\t0", b"\\\t0", b"\\\t0"),
        (b"\\\r0", b"\\\r0", b"\\\r0"),
    ] {
        assert_eq!(
            clean(input, SlashZeroMode::Remove, removed.len()),
            Ok(removed.to_vec())
        );
        assert_eq!(
            clean(input, SlashZeroMode::Keep, kept.len()),
            Ok(kept.to_vec())
        );
    }
}

#[test]
fn slash_zero_runs_use_source_single_pass_semantics_and_do_not_decode_other_escapes() {
    for (input, expected) in [
        (b"\\0".as_slice(), b"".as_slice()),
        (b"\\\\000", b""),
        (b"\\0\\00", b""),
        (b"\\0\\", b"\\"),
        (b"0\\x0", b"0\\x0"),
        (b"\\x\\0", b"\\x"),
        (b"\\n\\r\\t&#0;", b"\\n\\r\\t&#0;"),
    ] {
        assert_eq!(
            clean(input, SlashZeroMode::Remove, expected.len()),
            Ok(expected.to_vec())
        );
    }
}

#[test]
fn final_cap_is_checked_after_removal_and_large_runs_need_no_intermediate_output() {
    assert_eq!(clean(b"", SlashZeroMode::Remove, 0), Ok(Vec::new()));
    assert_eq!(
        clean(&vec![0; 65536], SlashZeroMode::Keep, 0),
        Ok(Vec::new())
    );
    let mut removed = vec![b'\\'; 32768];
    removed.extend_from_slice(&vec![b'0'; 32768]);
    assert_eq!(clean(&removed, SlashZeroMode::Remove, 0), Ok(Vec::new()));
    assert_eq!(
        clean(&removed, SlashZeroMode::Keep, removed.len()),
        Ok(removed.clone())
    );
    assert_eq!(
        clean(&removed, SlashZeroMode::Keep, removed.len() - 1),
        Err(NoNullError::OutputLimit)
    );
    let retained = vec![b'\\'; 65536];
    assert_eq!(
        clean(&retained, SlashZeroMode::Remove, retained.len()),
        Ok(retained.clone())
    );
    assert_eq!(
        clean(&retained, SlashZeroMode::Remove, retained.len() - 1),
        Err(NoNullError::OutputLimit)
    );
}

#[test]
fn raw_binary_input_is_immutable_and_default_mode_is_remove() {
    assert_eq!(SlashZeroMode::default(), SlashZeroMode::Remove);
    let input = b"\0\\0\xff\r\n\t\x7f".to_vec();
    let original = input.clone();
    let removed = b"\xff\r\n\t\x7f";
    let kept = b"\\0\xff\r\n\t\x7f";
    assert_eq!(
        clean(&input, SlashZeroMode::default(), removed.len()),
        Ok(removed.to_vec())
    );
    assert_eq!(
        clean(&input, SlashZeroMode::Keep, kept.len()),
        Ok(kept.to_vec())
    );
    assert_eq!(input, original);
}

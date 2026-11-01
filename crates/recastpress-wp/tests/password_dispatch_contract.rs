use recastpress_wp::{
    HostMaxBcryptCost, HostMaxPhpassCost, HostPasswordWorkLimits, PasswordDispatchError,
    wp_check_password_raw_empty_filters_bytes as verify,
};

fn limits(bcrypt: u8, portable: u8) -> HostPasswordWorkLimits {
    HostPasswordWorkLimits::new(
        HostMaxBcryptCost::new(bcrypt).unwrap(),
        HostMaxPhpassCost::new(portable).unwrap(),
    )
}

#[test]
fn actual_helpers_verify_public_dummy_vectors_without_trimming_nul_or_invalid_utf8() {
    let limits = limits(4, 7);
    for (raw, hash) in [
        (
            b"\0\xffpublic\x80dummy".as_slice(),
            b"43168edd77a7a6cddfbb37a2402c0d63".as_slice(),
        ),
        (
            b"\0\xffpublic\x80dummy",
            b"$P$5./09AZaz2gimYXfzYGLpJD2anG/Zf0",
        ),
        (
            b"dummy\0\xffpass",
            b"$wp$2y$04$......................S3bG01U.z0mtO6foDhFLmZTx5V6GN5K",
        ),
    ] {
        let saved_raw = raw.to_vec();
        let saved_hash = hash.to_vec();
        assert_eq!(verify(raw, hash, limits), Ok(true));
        assert_eq!(verify(b"wrong public dummy", hash, limits), Ok(false));
        assert_eq!(raw, saved_raw);
        assert_eq!(hash, saved_hash);
    }
    let spaced = b" \tpublic-dummy\0 ";
    let hash = b"$P$5salt1234V9bkU6TOOK2Vdi9Elmjy81";
    assert_eq!(verify(spaced, hash, limits), Ok(true));
    assert_eq!(verify(b"public-dummy", hash, limits), Ok(false));
    assert_eq!(limits.bcrypt_maximum().get(), 4);
    assert_eq!(limits.portable_maximum().get(), 7);
}

#[test]
fn md5_hash_length_priority_and_raw_length_guard_precede_unsupported_format_parsing() {
    let low = limits(4, 7);
    let long = vec![b'x'; 4097];
    assert_eq!(
        verify(&long, b"88751dd296563726278c2d57ab7549e4", low),
        Ok(true)
    );
    for hash in [b"".as_slice(), b"$P$", b"$wp", &[255; 32]] {
        assert_eq!(verify(&long, hash, low), Ok(false));
        assert_eq!(verify(b"short", hash, low), Ok(false));
    }
    for hash in [
        vec![255; 33],
        b"$wp malformed noncanonical unsupported suffix".to_vec(),
        b"$P$!........invalid portable body invalid".to_vec(),
        b"$H$B12345678ymVq6hVWkVcT1b2.4Yw6B.".to_vec(),
        b"$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C".to_vec(),
    ] {
        assert!(hash.len() > 32);
        assert_eq!(verify(&long, &hash, low), Ok(false));
        assert_eq!(
            verify(&[b'x'; 4096], &hash, low),
            Err(PasswordDispatchError::UnsupportedHash)
        );
    }
}

#[test]
fn algorithm_cost_refusals_are_distinct_from_wrong_password_and_the_long_guard() {
    let low = limits(4, 7);
    for (hash, error) in [
        (
            b"$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C".as_slice(),
            PasswordDispatchError::BcryptCostLimit,
        ),
        (
            b"$P$B12345678ymVq6hVWkVcT1b2.4Yw6B.",
            PasswordDispatchError::PortableCostLimit,
        ),
    ] {
        assert_eq!(verify(b"public dummy", hash, low), Err(error));
        assert_eq!(verify(&[0; 4097], hash, low), Ok(false));
    }
    assert_eq!(
        verify(
            b"dummy-pass",
            b"$wp$2y$04$......................p4I2i5y/qZ1mBr4uEKqha7UHXK0Clmq",
            low
        ),
        Ok(true)
    );
}

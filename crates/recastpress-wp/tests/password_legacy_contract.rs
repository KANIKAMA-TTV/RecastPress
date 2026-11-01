use recastpress_wp::{
    HostMaxPhpassCost, LegacyPasswordVerifyError, wp_check_password_legacy_md5_bytes as md5_verify,
    wp_check_password_phpass_portable_bytes as portable_verify,
};

#[test]
fn md5_source_branch_uses_every_raw_byte_before_any_4096_guard_and_lowercase_exact_hex() {
    // Public dummy outputs observed with the pinned PHP 8.5.12 md5 function.
    let long = vec![b'x'; 4097];
    for (password, hash) in [
        (
            b"".as_slice(),
            b"d41d8cd98f00b204e9800998ecf8427e".as_slice(),
        ),
        (
            b"\0\xffpublic\x80dummy",
            b"43168edd77a7a6cddfbb37a2402c0d63",
        ),
        (b" \tpublic-dummy\0 ", b"b4548c0a11968a68897ef7252d96568a"),
        (long.as_slice(), b"88751dd296563726278c2d57ab7549e4"),
    ] {
        let saved_password = password.to_vec();
        let saved_hash = hash.to_vec();
        assert_eq!(md5_verify(password, hash), Ok(true));
        assert_eq!(md5_verify(password, &hash.to_ascii_uppercase()), Ok(false));
        assert_eq!(md5_verify(b"wrong public dummy", hash), Ok(false));
        assert_eq!(password, saved_password);
        assert_eq!(hash, saved_hash);
    }
    assert_eq!(md5_verify(b"anything", b"0"), Ok(false));
    assert_eq!(md5_verify(b"anything", b""), Ok(false));
    assert_eq!(md5_verify(b"anything", &[255; 32]), Ok(false));
    assert_eq!(
        md5_verify(b"anything", &[b'0'; 33]),
        Err(LegacyPasswordVerifyError::UnsupportedHash)
    );
}

#[test]
fn pinned_php_portable_vectors_cover_raw_binary_trim_difference_stored_cost_and_length_boundary() {
    // Actual fixed class-phpass.php crypt_private outputs; no generated secrets.
    let max = HostMaxPhpassCost::new(13).unwrap();
    let boundary = vec![b'x'; 4096];
    for (password, hash) in [
        (
            b"".as_slice(),
            b"$P$512345678YsQn8dJMnXIqYTKyUqL7q1".as_slice(),
        ),
        (
            b"public-legacy-dummy",
            b"$P$512345678X6j1oU8AetC4zjJyp/36r1",
        ),
        (
            b"\0\xffpublic\x80dummy",
            b"$P$5./09AZaz2gimYXfzYGLpJD2anG/Zf0",
        ),
        (b" \tpublic-dummy\0 ", b"$P$5salt1234V9bkU6TOOK2Vdi9Elmjy81"),
        (
            b"public-legacy-dummy",
            b"$P$B12345678ymVq6hVWkVcT1b2.4Yw6B.",
        ),
        (boundary.as_slice(), b"$P$512345678PeS8FEIfNJzWuqAFIL7wz/"),
    ] {
        let saved_password = password.to_vec();
        let saved_hash = hash.to_vec();
        assert_eq!(portable_verify(password, hash, max), Ok(true));
        assert_eq!(portable_verify(b"wrong public dummy", hash, max), Ok(false));
        assert_eq!(password, saved_password);
        assert_eq!(hash, saved_hash);
    }
    assert_eq!(
        portable_verify(b"public-dummy", b"$P$5salt1234V9bkU6TOOK2Vdi9Elmjy81", max),
        Ok(false)
    );
    assert_eq!(
        portable_verify(&[b'x'; 4097], b"$P$512345678PeS8FEIfNJzWuqAFIL7wz/", max),
        Ok(false)
    );
}

#[test]
fn explicit_profile_refuses_crypt_fallback_and_noncanonical_or_excess_cost_separately_from_mismatch()
 {
    let low = HostMaxPhpassCost::new(7).unwrap();
    assert_eq!(
        portable_verify(b"dummy", b"$P$B12345678ymVq6hVWkVcT1b2.4Yw6B.", low),
        Err(LegacyPasswordVerifyError::CostLimit)
    );
    let canonical = b"$P$512345678X6j1oU8AetC4zjJyp/36r1";
    for (index, byte) in [(1, b'H'), (3, b'T'), (4, 0), (12, 255), (33, b'5')] {
        let mut unsupported = *canonical;
        unsupported[index] = byte;
        assert_eq!(
            portable_verify(b"public-legacy-dummy", &unsupported, low),
            Err(LegacyPasswordVerifyError::UnsupportedHash)
        );
        // Envelope admission precedes the source raw length guard; body parsing
        // follows it. A different prefix remains outside this explicit API.
        assert_eq!(
            portable_verify(&[0; 4097], &unsupported, low),
            if index == 1 {
                Err(LegacyPasswordVerifyError::UnsupportedHash)
            } else {
                Ok(false)
            }
        );
    }
    for hash in [
        canonical[..33].to_vec(),
        [canonical.as_slice(), b"x"].concat(),
    ] {
        assert_eq!(
            portable_verify(b"dummy", &hash, low),
            Err(LegacyPasswordVerifyError::UnsupportedHash)
        );
    }
    // Unknown cost cannot cause a shift, and no default host cost exists.
    for cost in [0, 6, 31, 255] {
        assert!(HostMaxPhpassCost::new(cost).is_err());
    }
}

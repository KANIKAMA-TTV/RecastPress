use recastpress_wp::{
    HostMaxBcryptCost, WpBcryptVerifyError, wp_check_password_wp_bcrypt_bytes as verify,
};
const DUMMY: &[u8] = b"$wp$2y$04$......................p4I2i5y/qZ1mBr4uEKqha7UHXK0Clmq";
const BINARY: &[u8] = b"$wp$2y$04$......................S3bG01U.z0mtO6foDhFLmZTx5V6GN5K";
const COST12: &[u8] = b"$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C";
#[test]
fn fixed_php_vectors_verify_raw_bytes_without_trimming_or_nul_and_utf8_coercion() {
    let max = HostMaxBcryptCost::new(4).unwrap();
    assert_eq!(verify(b"dummy-pass", DUMMY, max), Ok(true));
    for password in [
        b"wrong".as_slice(),
        b" dummy-pass",
        b"dummy-pass ",
        b"dummy-pass\0tail",
    ] {
        assert_eq!(verify(password, DUMMY, max), Ok(false));
    }
    let password = b"dummy\0\xffpass";
    let saved = password.to_vec();
    assert_eq!(verify(password, BINARY, max), Ok(true));
    assert_eq!(verify(b"dummy", BINARY, max), Ok(false));
    assert_eq!(password.as_slice(), saved);
}
#[test]
fn length_boundary_and_explicit_host_cost_preserve_false_versus_refusal() {
    let max = HostMaxBcryptCost::new(4).unwrap();
    let full = b"$wp$2y$04$......................PJ0SHzxZ5XVAIaQ.o5XGE7.jlbWF7K2";
    assert_eq!(verify(&[b'x'; 4096], full, max), Ok(true));
    assert_eq!(verify(&[b'x'; 4097], full, max), Ok(false));
    assert_eq!(
        verify(b"dummy-pass", COST12, max),
        Err(WpBcryptVerifyError::CostLimit)
    );
    assert_eq!(verify(&[b'x'; 4097], COST12, max), Ok(false));
    assert_eq!(
        verify(b"dummy-pass", COST12, HostMaxBcryptCost::new(12).unwrap()),
        Ok(true)
    );
}
#[test]
fn unsupported_algorithms_and_malformed_noncanonical_hashes_are_not_password_mismatches() {
    let max = HostMaxBcryptCost::new(4).unwrap();
    for hash in [
        b"5f4dcc3b5aa765d61d8327deb882cf99".as_slice(),
        &DUMMY[3..],
        b"$P$legacy",
        b"$argon2id$unsupported",
        b"$wp$2y$04$short",
    ] {
        assert_eq!(
            verify(b"dummy", hash, max),
            Err(WpBcryptVerifyError::UnsupportedHash)
        );
    }
    for index in [0, 3, 4, 6, 9, 10, 31, 62] {
        let mut bad = DUMMY.to_vec();
        bad[index] = b'!';
        assert_eq!(
            verify(b"dummy", &bad, max),
            Err(WpBcryptVerifyError::UnsupportedHash)
        );
    }
    let mut noncanonical = DUMMY.to_vec();
    noncanonical[31] = b'/';
    assert_eq!(
        verify(b"dummy", &noncanonical, max),
        Err(WpBcryptVerifyError::UnsupportedHash)
    );
}

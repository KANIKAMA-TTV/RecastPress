use recastpress_wp::{
    HostMaxBcryptCost, WpPasswordHashError, WpPasswordHashOutcome,
    wp_check_password_wp_bcrypt_bytes as verify, wp_hash_password_default_bcrypt_bytes as generate,
};

#[test]
fn os_random_twoy_cost12_hash_cross_verifies_trimmed_binary_input_without_mutation() {
    let password = b" \t\0dummy\0\xffpass\r\n\x0b";
    let saved = password.to_vec();
    let max = HostMaxBcryptCost::new(12).unwrap();
    let outcome = generate(password, max).unwrap();
    let WpPasswordHashOutcome::Hash(hash) = outcome else {
        panic!("bounded input must generate a hash");
    };
    let stored = hash.as_wp_storage_bytes();
    assert_eq!(stored.len(), 63);
    assert!(stored.starts_with(b"$wp$2y$12$"));
    assert!(stored.is_ascii());
    assert_eq!(verify(b"dummy\0\xffpass", stored, max), Ok(true));
    assert_eq!(verify(password, stored, max), Ok(false));
    assert_eq!(verify(b"wrong dummy", stored, max), Ok(false));
    assert_eq!(password.as_slice(), saved);
    assert!(!format!("{hash:?}").contains(std::str::from_utf8(stored).unwrap()));
}

#[test]
fn source_overlength_marker_and_host_refusal_are_distinct_without_rng_requirement() {
    let low = HostMaxBcryptCost::new(4).unwrap();
    let long = generate(&[b' '; 4097], low).unwrap();
    assert!(matches!(long, WpPasswordHashOutcome::PasswordTooLong));
    assert_eq!(long.as_wp_storage_bytes(), b"*");
    assert!(matches!(
        generate(b"", low),
        Err(WpPasswordHashError::CostLimit)
    ));
}

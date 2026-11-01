use recastpress_wp::{CompatibilityError, wp_password_needs_rehash_default_bcrypt_bytes as rehash};

const BASE: &[u8] = b"$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C";

#[test]
fn nonprefixed_bytes_are_true_without_trimming_or_hash_recognition() {
    for input in [
        b"".as_slice(),
        b"*",
        &BASE[3..],
        b"$WP",
        b" $wp",
        b"\0$wp",
        b"\xff",
    ] {
        assert_eq!(rehash(input), Ok(true));
    }
    for byte in 0..=255 {
        let input = [byte, 0, 255];
        assert_eq!(rehash(&input), Ok(true));
    }
    assert_eq!(rehash(&vec![b'x'; 8192]), Ok(true));
}

#[test]
fn every_canonical_cost_is_metadata_only_and_twelve_alone_is_false() {
    for cost in 4..=31 {
        let mut input = BASE.to_vec();
        input[7] = b'0' + cost / 10;
        input[8] = b'0' + cost % 10;
        let before = input.clone();
        assert_eq!(rehash(&input), Ok(cost != 12));
        assert_eq!(input, before);
    }
}

#[test]
fn prefixed_loose_php_recognition_is_not_silently_admitted() {
    let mut invalid = vec![b"$wp".to_vec(), BASE[..62].to_vec(), [BASE, b"\n"].concat()];
    for (index, byte) in [
        (6, b'b'),
        (7, b'X'),
        (8, b'X'),
        (9, b'X'),
        (10, b'!'),
        (10, 0),
        (10, 255),
        (31, b'/'),
        (62, b'/'),
    ] {
        let mut input = BASE.to_vec();
        input[index] = byte;
        invalid.push(input);
    }
    for cost in [b"00", b"03", b"32", b"99"] {
        let mut input = BASE.to_vec();
        input[7..9].copy_from_slice(cost);
        invalid.push(input);
    }
    for input in invalid {
        let before = input.clone();
        assert_eq!(rehash(&input), Err(CompatibilityError::UnsupportedValue));
        assert_eq!(input, before);
    }
}

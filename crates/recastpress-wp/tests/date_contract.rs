use recastpress_wp::wp_checkdate_integers;

#[test]
fn checkdate_keeps_gregorian_century_and_php_year_boundaries() {
    for (year, leap) in [
        (1600, true),
        (1700, false),
        (1900, false),
        (2000, true),
        (2024, true),
        (2100, false),
    ] {
        assert_eq!(
            wp_checkdate_integers(2, 29, year, b"not parsed", |valid, _| valid),
            leap
        );
    }
    for (month, day, year) in [
        (0, 1, 2024),
        (13, 1, 2024),
        (4, 31, 2024),
        (1, 0, 2024),
        (1, 1, 0),
        (1, 1, -1),
        (1, 1, 32768),
        (i64::MAX, i64::MIN, i64::MIN),
    ] {
        assert!(!wp_checkdate_integers(month, day, year, b"", |valid, _| {
            valid
        }));
    }
    assert!(wp_checkdate_integers(12, 31, 32767, b"", |valid, _| valid));
}

#[test]
fn checkdate_filter_receives_original_bytes_and_can_override_the_result() {
    let result = wp_checkdate_integers(2, 30, 2024, b"source\0\xff", |valid, source| {
        assert!(!valid);
        assert_eq!(source, b"source\0\xff");
        42
    });
    assert_eq!(result, 42);
}

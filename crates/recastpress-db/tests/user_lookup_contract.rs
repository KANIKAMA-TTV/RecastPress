use recastpress_db::{UserLookupError, UserRow};
use std::error::Error;

#[test]
fn lookup_errors_have_no_driver_source_or_private_fields() {
    for error in [
        UserLookupError::ProfileMismatch,
        UserLookupError::QueryFailure,
        UserLookupError::RowDecodeFailure,
    ] {
        assert!(error.source().is_none());
        let text = format!("{error:?}: {error}");
        assert!(!text.contains("private-key"));
        assert!(!text.contains("sqlx"));
    }
}

#[test]
fn lookup_result_uses_existing_id_only_debug() {
    let private = b"private-key-password-activation-contact".to_vec();
    let user = UserRow {
        id: 7,
        user_login: private.clone(),
        user_pass: private.clone(),
        user_nicename: private.clone(),
        user_email: private.clone(),
        user_url: private.clone(),
        user_registered: "0000-00-00 00:00:00".into(),
        user_activation_key: private.clone(),
        user_status: i32::MAX,
        display_name: private,
    };
    assert_eq!(format!("{user:?}"), "UserRow { id: 7, .. }");
}

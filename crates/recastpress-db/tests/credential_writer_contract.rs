use recastpress_db::{UserCredentialFields, UserCredentialWriteError};
use std::error::Error;

#[test]
fn explicit_raw_replacement_and_closed_diagnostics_do_not_disclose_password_or_activation_key() {
    let fields = UserCredentialFields {
        user_pass: b"private-PASSWORD\0\xff".to_vec(),
        user_activation_key: b"private-ACTIVATION\0".to_vec(),
    };
    assert_eq!(format!("{fields:?}"), "UserCredentialFields { .. }");
    let copied = fields.clone();
    assert_eq!(fields, copied);
    assert_eq!(fields.user_pass, b"private-PASSWORD\0\xff");
    assert_eq!(fields.user_activation_key, b"private-ACTIVATION\0");
    for error in [
        UserCredentialWriteError::Database,
        UserCredentialWriteError::StorageMismatch,
        UserCredentialWriteError::CommitOutcomeUnknown,
    ] {
        let text = format!("{error:?}: {error}");
        assert!(!text.contains("private-PASSWORD"));
        assert!(!text.contains("private-ACTIVATION"));
        assert!(!text.contains("sqlx"));
        assert!(error.source().is_none());
    }
}

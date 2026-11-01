use recastpress_db::{LinkRow, UserMetaRow, UserRow};

#[test]
fn account_debug_does_not_include_credentials_identity_or_capabilities() {
    let secret = b"private-fixture".to_vec();
    let user = UserRow {
        id: 1,
        user_login: secret.clone(),
        user_pass: secret.clone(),
        user_nicename: secret.clone(),
        user_email: secret.clone(),
        user_url: secret.clone(),
        user_registered: "0000-00-00 00:00:00".into(),
        user_activation_key: secret.clone(),
        user_status: 0,
        display_name: secret.clone(),
    };
    let meta = UserMetaRow {
        umeta_id: 2,
        user_id: 1,
        meta_key: Some(secret.clone()),
        meta_value: Some(secret),
    };
    for debug in [format!("{user:?}"), format!("{meta:?}")] {
        assert!(!debug.contains("private-fixture"));
        assert!(!debug.contains("112, 114, 105"));
    }
}

#[test]
fn legacy_link_debug_omits_stored_text() {
    let secret = b"private-fixture".to_vec();
    let link = LinkRow {
        link_id: 1,
        link_url: secret.clone(),
        link_name: secret.clone(),
        link_image: secret.clone(),
        link_target: secret.clone(),
        link_description: secret.clone(),
        link_visible: secret.clone(),
        link_owner: 2,
        link_rating: 0,
        link_updated: "0000-00-00 00:00:00".into(),
        link_rel: secret.clone(),
        link_notes: secret.clone(),
        link_rss: secret,
    };
    let debug = format!("{link:?}");
    assert!(!debug.contains("private-fixture"));
    assert!(!debug.contains("112, 114, 105"));
}

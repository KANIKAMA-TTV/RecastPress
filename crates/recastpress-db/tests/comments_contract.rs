use recastpress_db::{CommentMetaRow, CommentRow};

#[test]
fn comment_debug_omits_contact_information_content_and_metadata() {
    let secret = b"private-fixture".to_vec();
    let comment = CommentRow {
        comment_id: 1,
        comment_post_id: 2,
        comment_author: secret.clone(),
        comment_author_email: secret.clone(),
        comment_author_url: secret.clone(),
        comment_author_ip: secret.clone(),
        comment_date: "0000-00-00 00:00:00".into(),
        comment_date_gmt: "0000-00-00 00:00:00".into(),
        comment_content: secret.clone(),
        comment_karma: 0,
        comment_approved: secret.clone(),
        comment_agent: secret.clone(),
        comment_type: secret.clone(),
        comment_parent: 0,
        user_id: 0,
    };
    let meta = CommentMetaRow {
        meta_id: 3,
        comment_id: 1,
        meta_key: Some(secret.clone()),
        meta_value: Some(secret),
    };
    for debug in [format!("{comment:?}"), format!("{meta:?}")] {
        assert!(!debug.contains("private-fixture"));
        assert!(!debug.contains("112, 114, 105"));
    }
}

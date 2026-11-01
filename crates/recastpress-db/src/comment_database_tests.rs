//! Called only after the disposable DB harness's existing identity checks.
use super::*;
use sqlx::mysql::MySqlConnection;

pub(super) async fn contract(
    repository: &EditingRepository,
    writer: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut expected = Vec::new();
    for (index, (id, status, kind)) in [
        (0u64, "0", "comment"),
        (1, "1", "comment"),
        (2, "spam", "pingback"),
        (3, "trash", "trackback"),
        (u64::MAX, "plugin_review", "plugin_type"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = CommentRow {
            comment_id: id,
            comment_post_id: 999,
            comment_author: "架空の投稿者\0🦀".as_bytes().to_vec(),
            comment_author_email: b"fixture@example.invalid".to_vec(),
            comment_author_url: b"https://example.invalid/?a='".to_vec(),
            comment_author_ip: b"2001:db8::1\0fixture".to_vec(),
            comment_date: if index == 1 {
                "2020-02-29 12:34:56"
            } else {
                "0000-00-00 00:00:00"
            }
            .into(),
            comment_date_gmt: "0000-00-00 00:00:00".into(),
            comment_content: "非公開の本文\0 ' OR 1=1 -- 🦀".as_bytes().to_vec(),
            comment_karma: if index % 2 == 0 { i32::MIN } else { i32::MAX },
            comment_approved: status.as_bytes().to_vec(),
            comment_agent: b"fixture-agent\0".to_vec(),
            comment_type: kind.as_bytes().to_vec(),
            comment_parent: u64::MAX,
            user_id: u64::MAX,
        };
        sqlx::query("INSERT INTO wp_comments (comment_ID,comment_post_ID,comment_author,comment_author_email,comment_author_url,comment_author_IP,comment_date,comment_date_gmt,comment_content,comment_karma,comment_approved,comment_agent,comment_type,comment_parent,user_id) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(row.comment_id).bind(row.comment_post_id).bind(&row.comment_author).bind(&row.comment_author_email)
            .bind(&row.comment_author_url).bind(&row.comment_author_ip).bind(&row.comment_date).bind(&row.comment_date_gmt)
            .bind(&row.comment_content).bind(row.comment_karma).bind(&row.comment_approved).bind(&row.comment_agent)
            .bind(&row.comment_type).bind(row.comment_parent).bind(row.user_id).execute(&mut *writer).await?;
        expected.push(row);
    }
    let serial = b"a:1:{s:1:\"k\";s:3:\"a\0b\";}";
    for (id, comment, key, value) in [
        (0u64, 0u64, None, None),
        (1, 1, Some(b"".as_slice()), Some(b"".as_slice())),
        (2, 1, Some(b"duplicate".as_slice()), Some(serial.as_slice())),
        (
            3,
            1,
            Some(b"duplicate".as_slice()),
            Some(b"second".as_slice()),
        ),
        (
            u64::MAX,
            999,
            Some(b"orphan".as_slice()),
            Some(b"keep me".as_slice()),
        ),
    ] {
        sqlx::query(
            "INSERT INTO wp_commentmeta (meta_id,comment_id,meta_key,meta_value) VALUES (?,?,?,?)",
        )
        .bind(id)
        .bind(comment)
        .bind(key)
        .bind(value)
        .execute(&mut *writer)
        .await?;
    }
    let one = NonZeroU16::new(1).unwrap();
    let two = NonZeroU16::new(2).unwrap();
    let ten = NonZeroU16::new(10).unwrap();
    let mut snapshot = repository.snapshot().await?;
    let mut after = None;
    let mut actual = Vec::new();
    loop {
        let page = snapshot.comments_page(after, two).await?;
        if page.is_empty() {
            break;
        }
        assert!(page.len() <= 2);
        assert!(after.is_none_or(|id| page[0].comment_id > id));
        after = page.last().map(|row| row.comment_id);
        actual.extend(page);
        assert!(actual.len() <= expected.len());
    }
    assert_eq!(actual, expected);
    assert_eq!(snapshot.comment(0).await?.as_ref(), Some(&expected[0]));
    assert_eq!(
        snapshot.comment(u64::MAX).await?.as_ref(),
        Some(&expected[4])
    );
    assert!(snapshot.comment(99).await?.is_none());
    assert_eq!(snapshot.comments_page(Some(0), one).await?[0].comment_id, 1);
    let meta = snapshot.comment_meta_page(None, ten).await?;
    assert_eq!(meta.len(), 5);
    assert_eq!(meta[0].meta_id, 0);
    assert_eq!((&meta[0].meta_key, &meta[0].meta_value), (&None, &None));
    assert_eq!(
        (&meta[1].meta_key, &meta[1].meta_value),
        (&Some(vec![]), &Some(vec![]))
    );
    assert_eq!(meta[2].meta_key, meta[3].meta_key);
    assert_eq!(meta[2].meta_value.as_deref(), Some(serial.as_slice()));
    assert_eq!(meta[4].comment_id, 999);
    assert_eq!(
        snapshot.comment_meta_page(Some(0), one).await?[0].meta_id,
        1
    );
    assert!(
        snapshot
            .comment_meta_page(Some(u64::MAX), one)
            .await?
            .is_empty()
    );
    sqlx::query("UPDATE wp_comments SET comment_content='changed',comment_approved='plugin_changed' WHERE comment_ID=1").execute(&mut *writer).await?;
    sqlx::query("UPDATE wp_commentmeta SET meta_value='changed' WHERE meta_id=2")
        .execute(&mut *writer)
        .await?;
    assert_eq!(snapshot.comment(1).await?.as_ref(), Some(&expected[1]));
    assert_eq!(snapshot.comment_meta_page(Some(1), one).await?[0], meta[2]);
    snapshot.finish().await?;
    let mut newer = repository.snapshot().await?;
    let changed = newer.comment(1).await?.unwrap();
    assert_eq!(changed.comment_content, b"changed");
    assert_eq!(changed.comment_approved, b"plugin_changed");
    assert_eq!(
        newer.comment_meta_page(Some(1), one).await?[0]
            .meta_value
            .as_deref(),
        Some(b"changed".as_slice())
    );
    newer.finish().await?;
    for table in ["wp_comments", "wp_commentmeta"] {
        QueryBuilder::<MySql>::new("ALTER TABLE ")
            .push(table)
            .push(" ENGINE=MyISAM")
            .build()
            .execute(&mut *writer)
            .await?;
        match repository.snapshot().await {
            Err(sqlx::Error::Configuration(error)) => assert!(
                matches!(error.downcast_ref::<SnapshotSchemaError>(),Some(SnapshotSchemaError::NonTransactionalTable { table:name,.. }) if name==table)
            ),
            _ => panic!("comment nontransactional table was not rejected"),
        }
        QueryBuilder::<MySql>::new("ALTER TABLE ")
            .push(table)
            .push(" ENGINE=InnoDB")
            .build()
            .execute(&mut *writer)
            .await?;
        QueryBuilder::<MySql>::new("RENAME TABLE ")
            .push(table)
            .push(" TO ")
            .push(table)
            .push("_fixture_hidden")
            .build()
            .execute(&mut *writer)
            .await?;
        match repository.snapshot().await {
            Err(sqlx::Error::Configuration(error)) => assert!(
                matches!(error.downcast_ref::<SnapshotSchemaError>(),Some(SnapshotSchemaError::MissingTable { table:name }) if name==table)
            ),
            _ => panic!("comment missing table was not rejected"),
        }
        QueryBuilder::<MySql>::new("RENAME TABLE ")
            .push(table)
            .push("_fixture_hidden TO ")
            .push(table)
            .build()
            .execute(&mut *writer)
            .await?;
    }
    Ok(())
}

//! Additional legacy/account retention checks in the already-owned fixture.
use super::*;
use sqlx::mysql::MySqlConnection;

pub(super) async fn contract(
    repository: &EditingRepository,
    writer: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut users = Vec::new();
    let mut links = Vec::new();
    for (index, id) in [0u64, 1, u64::MAX].into_iter().enumerate() {
        let user = UserRow {
            id,
            user_login: b"fixture-login\0".to_vec(),
            user_pass: b"fixture-NOT-A-PASSWORD-HASH\0".to_vec(),
            user_nicename: b"Case-Sensitive".to_vec(),
            user_email: b"fixture@example.invalid".to_vec(),
            user_url: b"https://example.invalid/".to_vec(),
            user_registered: if index == 1 {
                "2020-02-29 12:34:56"
            } else {
                "0000-00-00 00:00:00"
            }
            .into(),
            user_activation_key: b"fixture-NOT-AN-ACTIVATION-KEY\0".to_vec(),
            user_status: if index % 2 == 0 { i32::MIN } else { i32::MAX },
            display_name: "非公開の表示名\0🦀".as_bytes().to_vec(),
        };
        sqlx::query("INSERT INTO wp_users (ID,user_login,user_pass,user_nicename,user_email,user_url,user_registered,user_activation_key,user_status,display_name) VALUES (?,?,?,?,?,?,?,?,?,?)")
            .bind(user.id).bind(&user.user_login).bind(&user.user_pass).bind(&user.user_nicename).bind(&user.user_email).bind(&user.user_url)
            .bind(&user.user_registered).bind(&user.user_activation_key).bind(user.user_status).bind(&user.display_name).execute(&mut *writer).await?;
        users.push(user);
        let link = LinkRow {
            link_id: id,
            link_url: b"https://example.invalid/?a=' OR 1=1 --".to_vec(),
            link_name: "古いリンク\0🦀".as_bytes().to_vec(),
            link_image: b"image\0".to_vec(),
            link_target: b"_blank".to_vec(),
            link_description: b"description\0".to_vec(),
            link_visible: [b"Y".as_slice(), b"N".as_slice(), b"plugin_state".as_slice()][index]
                .to_vec(),
            link_owner: u64::MAX,
            link_rating: if index % 2 == 0 { i32::MIN } else { i32::MAX },
            link_updated: if index == 1 {
                "2020-02-29 12:34:56"
            } else {
                "0000-00-00 00:00:00"
            }
            .into(),
            link_rel: b"friend\0custom".to_vec(),
            link_notes: "保持するメモ\0🦀".as_bytes().to_vec(),
            link_rss: b"https://example.invalid/feed".to_vec(),
        };
        sqlx::query("INSERT INTO wp_links (link_id,link_url,link_name,link_image,link_target,link_description,link_visible,link_owner,link_rating,link_updated,link_rel,link_notes,link_rss) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(link.link_id).bind(&link.link_url).bind(&link.link_name).bind(&link.link_image).bind(&link.link_target).bind(&link.link_description)
            .bind(&link.link_visible).bind(link.link_owner).bind(link.link_rating).bind(&link.link_updated).bind(&link.link_rel).bind(&link.link_notes).bind(&link.link_rss).execute(&mut *writer).await?;
        links.push(link);
    }
    let capabilities = b"a:1:{s:13:\"administrator\";b:1;}";
    for (id, user, key, value) in [
        (0u64, 0u64, None, None),
        (1, 1, Some(b"".as_slice()), Some(b"".as_slice())),
        (
            2,
            1,
            Some(b"wp_capabilities".as_slice()),
            Some(capabilities.as_slice()),
        ),
        (
            3,
            1,
            Some(b"wp_capabilities".as_slice()),
            Some(b"a:0:{}".as_slice()),
        ),
        (
            u64::MAX,
            999,
            Some(b"orphan".as_slice()),
            Some(b"keep me".as_slice()),
        ),
    ] {
        sqlx::query(
            "INSERT INTO wp_usermeta (umeta_id,user_id,meta_key,meta_value) VALUES (?,?,?,?)",
        )
        .bind(id)
        .bind(user)
        .bind(key)
        .bind(value)
        .execute(&mut *writer)
        .await?;
    }
    let one = NonZeroU16::new(1).unwrap();
    let ten = NonZeroU16::new(10).unwrap();
    let mut snapshot = repository.snapshot().await?;
    assert_eq!(snapshot.users_page(None, ten).await?, users);
    assert_eq!(snapshot.links_page(None, ten).await?, links);
    let mut after = None;
    for (user, link) in users.iter().zip(&links) {
        let user_page = snapshot.users_page(after, one).await?;
        let link_page = snapshot.links_page(after, one).await?;
        assert_eq!(user_page.as_slice(), std::slice::from_ref(user));
        assert_eq!(link_page.as_slice(), std::slice::from_ref(link));
        after = Some(user.id);
    }
    assert!(snapshot.users_page(after, one).await?.is_empty());
    assert!(snapshot.links_page(after, one).await?.is_empty());
    let meta = snapshot.user_meta_page(None, ten).await?;
    assert_eq!(meta.len(), 5);
    assert_eq!(meta[0].umeta_id, 0);
    assert_eq!((&meta[0].meta_key, &meta[0].meta_value), (&None, &None));
    assert_eq!(
        (&meta[1].meta_key, &meta[1].meta_value),
        (&Some(vec![]), &Some(vec![]))
    );
    assert_eq!(meta[2].meta_key, meta[3].meta_key);
    assert_eq!(meta[2].meta_value.as_deref(), Some(capabilities.as_slice()));
    assert_eq!(meta[4].user_id, 999);
    assert_eq!(snapshot.user_meta_page(Some(0), one).await?[0].umeta_id, 1);
    assert!(
        snapshot
            .user_meta_page(Some(u64::MAX), one)
            .await?
            .is_empty()
    );
    for query in [
        "UPDATE wp_users SET user_pass='changed',user_activation_key='changed' WHERE ID=1",
        "UPDATE wp_usermeta SET meta_value='changed' WHERE umeta_id=2",
        "UPDATE wp_links SET link_notes='changed',link_visible='changed' WHERE link_id=1",
    ] {
        sqlx::query(query).execute(&mut *writer).await?;
    }
    assert_eq!(snapshot.users_page(Some(0), one).await?[0], users[1]);
    assert_eq!(snapshot.user_meta_page(Some(1), one).await?[0], meta[2]);
    assert_eq!(snapshot.links_page(Some(0), one).await?[0], links[1]);
    snapshot.finish().await?;
    let mut newer = repository.snapshot().await?;
    let changed = newer.users_page(Some(0), one).await?;
    assert_eq!(changed[0].user_pass, b"changed");
    assert_eq!(changed[0].user_activation_key, b"changed");
    assert_eq!(
        newer.user_meta_page(Some(1), one).await?[0]
            .meta_value
            .as_deref(),
        Some(b"changed".as_slice())
    );
    let changed = newer.links_page(Some(0), one).await?;
    assert_eq!(changed[0].link_notes, b"changed");
    assert_eq!(changed[0].link_visible, b"changed");
    newer.finish().await?;
    for table in ["wp_users", "wp_usermeta", "wp_links"] {
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
            _ => panic!("retained nontransactional table was not rejected"),
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
            _ => panic!("retained missing table was not rejected"),
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

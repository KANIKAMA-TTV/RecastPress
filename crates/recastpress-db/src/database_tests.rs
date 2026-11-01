//! Executed only by an explicit disposable-database harness, never default tests.
use super::*;
use sqlx::{
    Connection,
    mysql::{MySqlConnection, MySqlSslMode},
};

#[tokio::test]
#[ignore = "requires the isolated database_smoke.py fixture"]
async fn raw_rows_snapshot_and_read_only_contract() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        std::env::var("RECASTPRESS_DB_FIXTURE").as_deref(),
        Ok("disposable")
    );
    let port = std::env::var("RECASTPRESS_DB_PORT")?.parse::<u16>()?;
    let password = std::env::var("RECASTPRESS_DB_PASSWORD")?;
    let mut options = MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username(&std::env::var("RECASTPRESS_DB_USER")?)
        .password(&password)
        .database("recastpress_fixture")
        .charset("utf8mb4")
        .collation("utf8mb4_unicode_ci")
        .ssl_mode(MySqlSslMode::Disabled);
    if let Ok(ca) = std::env::var("RECASTPRESS_DB_CA") {
        options = options.ssl_ca(ca).ssl_mode(MySqlSslMode::VerifyCa);
    }
    let mut writer = MySqlConnection::connect_with(&options).await?;
    sqlx::query("SET SESSION sql_mode = ''")
        .execute(&mut writer)
        .await?;
    let (version,): (String,) = sqlx::query_as("SELECT VERSION()")
        .fetch_one(&mut writer)
        .await?;
    let (_, tls_cipher): (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
        .fetch_one(&mut writer)
        .await?;
    if std::env::var("RECASTPRESS_DB_CA").is_ok() {
        assert!(
            !tls_cipher.is_empty(),
            "CA-verifying fixture connection did not negotiate TLS"
        );
    }
    assert!(version.contains(&std::env::var("RECASTPRESS_DB_EXPECT_VERSION")?));
    let title = "日本語 🦀 ' OR 1=1 --";
    let content = "content\0with NUL and emoji 🦀";
    let insert = "INSERT INTO wp_posts (ID,post_author,post_content,post_title,post_excerpt,to_ping,pinged,post_content_filtered,post_status,post_type,post_password,menu_order) VALUES (?,?,?,?,?,'','','',?,?,?,?)";
    for (id, status) in [
        (1u64, "private"),
        (2u64, "plugin_review"),
        (u64::MAX, "publish"),
    ] {
        sqlx::query(insert)
            .bind(id)
            .bind(u64::MAX)
            .bind(content)
            .bind(title)
            .bind("excerpt")
            .bind(status)
            .bind("post")
            .bind("fixture-password")
            .bind(-3i32)
            .execute(&mut writer)
            .await?;
    }
    let serialized = b"a:1:{s:3:\"key\";s:3:\"a\0b\";}".to_vec();
    let meta = "INSERT INTO wp_postmeta (meta_id,post_id,meta_key,meta_value) VALUES (?,?,?,?)";
    for (id, key, data) in [
        (1u64, None, None),
        (2, Some(Vec::new()), Some(Vec::new())),
        (3, Some(b"duplicate".to_vec()), Some(serialized.clone())),
        (4, Some(b"duplicate".to_vec()), Some(b"second".to_vec())),
    ] {
        sqlx::query(meta)
            .bind(id)
            .bind(1u64)
            .bind(key)
            .bind(data)
            .execute(&mut writer)
            .await?;
    }
    let option =
        "INSERT INTO wp_options (option_id,option_name,option_value,autoload) VALUES (?,?,?,?)";
    for (id, autoload) in [(1u64, "yes"), (2, "auto-on"), (3, "off")] {
        sqlx::query(option)
            .bind(id)
            .bind(format!("fixture_{id}"))
            .bind(serialized.clone())
            .bind(autoload)
            .execute(&mut writer)
            .await?;
    }
    sqlx::query(meta)
        .bind(5u64)
        .bind(999u64)
        .bind("orphan")
        .bind("keep me")
        .execute(&mut writer)
        .await?;
    let post_write_options = options.clone();
    let repository = EditingRepository::connect(options, TablePrefix::new("wp_").unwrap()).await?;
    let observed_binlog = repository.binlog_configuration().await?;
    let (enabled, format, row_image): (u64, String, String) = sqlx::query_as(
        "SELECT CAST(@@GLOBAL.log_bin AS UNSIGNED), @@GLOBAL.binlog_format, @@GLOBAL.binlog_row_image"
    ).fetch_one(&mut writer).await?;
    assert_eq!(
        observed_binlog,
        BinlogConfiguration {
            enabled: enabled != 0,
            format,
            row_image
        }
    );
    assert!(!observed_binlog.format.is_empty());
    assert!(!observed_binlog.row_image.is_empty());
    let mut snapshot = repository.snapshot().await?;
    if std::env::var("RECASTPRESS_DB_CA").is_ok() {
        let (_, cipher): (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
            .fetch_one(&mut *snapshot.transaction)
            .await?;
        assert!(!cipher.is_empty(), "editing pool did not negotiate TLS");
    }
    let post = snapshot.post(1).await?.unwrap();
    assert_eq!(post.post_title, title);
    assert_eq!(post.post_content, content);
    assert_eq!(post.post_status, "private");
    assert_eq!(post.post_author, u64::MAX);
    assert_eq!(post.menu_order, -3);
    assert_eq!(post.post_date, "0000-00-00 00:00:00");
    assert_eq!(post.post_date_gmt, "0000-00-00 00:00:00");
    assert_eq!(post.post_modified, "0000-00-00 00:00:00");
    assert!(!format!("{post:?}").contains("fixture-password"));
    assert!(snapshot.post(99).await?.is_none());
    let first = snapshot
        .posts_page(None, NonZeroU16::new(2).unwrap())
        .await?;
    assert_eq!(first.iter().map(|row| row.id).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(first[1].post_status, "plugin_review");
    let next = snapshot.posts_after(2, NonZeroU16::new(2).unwrap()).await?;
    assert_eq!(next[0].id, u64::MAX);
    assert!(
        snapshot
            .posts_after(u64::MAX, NonZeroU16::new(1).unwrap())
            .await?
            .is_empty()
    );
    let metadata = snapshot.post_meta(1).await?;
    assert_eq!(metadata.len(), 4);
    assert_eq!(metadata[0].meta_key, None);
    assert_eq!(metadata[0].meta_value, None);
    assert_eq!(metadata[1].meta_key, Some(Vec::new()));
    assert_eq!(metadata[1].meta_value, Some(Vec::new()));
    assert_eq!(metadata[2].meta_value, Some(serialized.clone()));
    assert_eq!(metadata[2].meta_key, metadata[3].meta_key);
    let all_metadata = snapshot
        .post_meta_after(4, NonZeroU16::new(10).unwrap())
        .await?;
    assert_eq!(all_metadata.len(), 1);
    assert_eq!(all_metadata[0].post_id, 999);
    let options = snapshot
        .options_page(None, NonZeroU16::new(10).unwrap())
        .await?;
    assert_eq!(
        options
            .iter()
            .map(|row| row.autoload.as_str())
            .collect::<Vec<_>>(),
        ["yes", "auto-on", "off"]
    );
    assert!(options.iter().all(|row| row.option_value == serialized));
    sqlx::query("UPDATE wp_posts SET post_title=? WHERE ID=?")
        .bind("changed outside snapshot")
        .bind(1u64)
        .execute(&mut writer)
        .await?;
    assert_eq!(snapshot.post(1).await?.unwrap().post_title, title);
    let forbidden = sqlx::query("UPDATE wp_posts SET post_title='must not write' WHERE ID=1")
        .execute(&mut *snapshot.transaction)
        .await;
    let read_only_code = match &forbidden {
        Err(sqlx::Error::Database(error)) => error
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .map(sqlx::mysql::MySqlDatabaseError::number),
        _ => None,
    };
    assert_eq!(
        read_only_code,
        Some(1792),
        "expected the server's read-only transaction rejection"
    );
    snapshot.finish().await?;
    let mut newer = repository.snapshot().await?;
    assert_eq!(
        newer.post(1).await?.unwrap().post_title,
        "changed outside snapshot"
    );
    newer.finish().await?;
    sqlx::query("ALTER TABLE wp_options ENGINE=MyISAM")
        .execute(&mut writer)
        .await?;
    let rejected = match repository.snapshot().await {
        Err(error) => error,
        Ok(_) => panic!("nontransactional table admitted as consistent snapshot"),
    };
    assert!(
        matches!(rejected,sqlx::Error::Configuration(ref error) if matches!(error.downcast_ref::<SnapshotSchemaError>(),Some(SnapshotSchemaError::NonTransactionalTable{..})))
    );
    sqlx::query("ALTER TABLE wp_options ENGINE=InnoDB")
        .execute(&mut writer)
        .await?;
    sqlx::query("RENAME TABLE wp_options TO wp_options_fixture_hidden")
        .execute(&mut writer)
        .await?;
    let rejected = match repository.snapshot().await {
        Err(error) => error,
        Ok(_) => panic!("missing table admitted as consistent snapshot"),
    };
    assert!(
        matches!(rejected,sqlx::Error::Configuration(ref error) if matches!(error.downcast_ref::<SnapshotSchemaError>(),Some(SnapshotSchemaError::MissingTable{..})))
    );
    sqlx::query("RENAME TABLE wp_options_fixture_hidden TO wp_options")
        .execute(&mut writer)
        .await?;
    // Stored zero IDs are possible with NO_AUTO_VALUE_ON_ZERO. This concerns
    // raw export completeness, not the WordPress API meaning of post ID zero.
    sqlx::query("SET SESSION sql_mode = 'NO_AUTO_VALUE_ON_ZERO'")
        .execute(&mut writer)
        .await?;
    sqlx::query(insert)
        .bind(0u64)
        .bind(0u64)
        .bind("zero content")
        .bind("zero title")
        .bind("")
        .bind("private")
        .bind("post")
        .bind("")
        .bind(0i32)
        .execute(&mut writer)
        .await?;
    sqlx::query(meta)
        .bind(0u64)
        .bind(0u64)
        .bind("zero key")
        .bind("zero value")
        .execute(&mut writer)
        .await?;
    sqlx::query(option)
        .bind(0u64)
        .bind("zero_option")
        .bind("zero value")
        .bind("off")
        .execute(&mut writer)
        .await?;
    let mut zero = repository.snapshot().await?;
    let two = NonZeroU16::new(2).unwrap();
    assert_eq!(
        zero.posts_page(None, two)
            .await?
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        zero.posts_page(Some(0), two)
            .await?
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        zero.posts_after(0, two)
            .await?
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        zero.post_meta_page(None, two)
            .await?
            .iter()
            .map(|row| row.meta_id)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        zero.post_meta_page(Some(0), two)
            .await?
            .iter()
            .map(|row| row.meta_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        zero.options_page(None, two)
            .await?
            .iter()
            .map(|row| row.option_id)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        zero.options_page(Some(0), two)
            .await?
            .iter()
            .map(|row| row.option_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    zero.finish().await?;
    taxonomy_contract(&repository, &mut writer).await?;
    super::comment_database_tests::contract(&repository, &mut writer).await?;
    super::retained_database_tests::contract(&repository, &mut writer).await?;
    user_lookup_contract(&repository, &mut writer).await?;
    post_name_lookup_contract(&repository, &mut writer).await?;
    post_name_preprocess_lookup_pipeline(&repository, &mut writer).await?;
    option_meta_selected_decode_pipeline(&repository, &mut writer).await?;
    user_credential_writer_contract(&repository, &mut writer, post_write_options.clone()).await?;
    credential_rehash_pipeline(&repository, &mut writer, post_write_options.clone()).await?;
    credential_preprocess_lookup_pipeline(&repository, &mut writer, post_write_options.clone())
        .await?;
    credential_username_email_stage_pipeline(&repository, &mut writer, post_write_options.clone())
        .await?;
    super::meta_view_database_tests::contract(&repository, &mut writer).await?;
    super::budgeted_meta_view_database_tests::contract(&repository, &mut writer).await?;
    post_text_writer_contract(&repository, &mut writer, post_write_options.clone()).await?;
    let alpha_inputs =
        if std::env::var("RECASTPRESS_ALPHA_POST_PATH").as_deref() == Ok("disposable") {
            Some(alpha_post_inputs(&repository, &mut writer, post_write_options).await?)
        } else {
            None
        };
    repository.close().await;
    writer.close().await?;
    let mut result = serde_json::json!({"server_version":version,"tls_cipher":tls_cipher,"checks":["unicode_and_nul","unsigned_id_boundary","private_and_custom_status","zero_dates","null_empty_duplicate_meta","orphan_meta","serialized_bytes","autoload_raw_values","cursor_pagination","repeatable_snapshot","read_only_enforcement","nontransactional_rejection","missing_table_rejection","binlog_configuration_observation","zero_id_first_page","taxonomy_raw_values","taxonomy_composite_cursor","taxonomy_repeatable_snapshot","taxonomy_schema_rejection","comment_raw_values","comment_cursor_and_snapshot","comment_schema_rejection","user_raw_values","user_cursor_and_snapshot","user_schema_rejection","link_raw_values","link_cursor_and_snapshot","link_schema_rejection","meta_index_binary_equivalence","meta_index_scan_fallback","meta_index_snapshot_isolation","meta_view_prepared_change_equivalence","meta_view_complete_paged_load","meta_view_limit_rejects_prefix","meta_view_byte_limit_exactness","meta_view_preflight_avoids_payload_transfer","budgeted_meta_view_shared_pool_lifetime","budgeted_meta_view_preflight_refusal","budgeted_meta_view_refusal_avoids_payload_transfer","post_text_cas_exactness","post_text_cas_conflict","post_text_cas_unsigned_zero_dates","post_text_cas_truncation_rollback","post_text_cas_concurrent_writers","post_text_cas_schema_rejection"]});
    result["checks"].as_array_mut().unwrap().extend([
        serde_json::json!("user_lookup_text_collation"),
        serde_json::json!("user_lookup_snapshot"),
        serde_json::json!("user_lookup_profile_rejection"),
        serde_json::json!("credential_cas_exactness"),
        serde_json::json!("credential_cas_conflict"),
        serde_json::json!("credential_cas_unsigned_zero_dates"),
        serde_json::json!("credential_cas_truncation_rollback"),
        serde_json::json!("credential_cas_concurrent_writers"),
        serde_json::json!("credential_cas_schema_rejection"),
        serde_json::json!("credential_rehash_pipeline"),
        serde_json::json!("credential_preprocess_lookup_pipeline"),
        serde_json::json!("credential_username_email_stage_pipeline"),
        serde_json::json!("post_name_text_collation"),
        serde_json::json!("post_name_text_pagination_snapshot"),
        serde_json::json!("post_name_text_profile_rejection"),
        serde_json::json!("post_name_preprocess_lookup_pipeline"),
        serde_json::json!("option_meta_selected_decode_pipeline"),
    ]);
    if let Some(inputs) = alpha_inputs {
        result.as_object_mut().unwrap().insert(
            "alpha_publication_process_pid".into(),
            serde_json::json!(std::process::id()),
        );
        result
            .as_object_mut()
            .unwrap()
            .insert("alpha_publication_inputs".into(), inputs);
    }
    println!("DB_CONTRACT_RESULT={result}");
    Ok(())
}

async fn option_meta_selected_decode_pipeline(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    use recastpress_wp::{
        serialization::{
            LimitedUnserializeError, UnserializeLimits,
            maybe_unserialize_bytes_with_limits as decode,
        },
        value::{PhpKey, PhpValue},
    };
    type ContractResult = Result<(), Box<dyn std::error::Error>>;
    let require = |condition: bool| -> ContractResult {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other("selected raw decode contract failed").into())
        }
    };
    let page = NonZeroU16::new(32).unwrap();
    // Before meta_view adds five rows: the original five plus stored ID0.
    let mut before = repository.snapshot().await?;
    let original_meta = before.post_meta_page(None, page).await?;
    let original_options = before.options_page(None, page).await?;
    before.finish().await?;
    require((original_meta.len(), original_options.len()) == (6, 4))?;
    require(
        original_meta
            .iter()
            .all(|row| !(1001..=1003).contains(&row.meta_id)),
    )?;
    let mut expected_stage = original_meta.clone();
    for (id, raw) in [
        (1001u64, b"N;".as_slice()),
        (1002, b"b:0;".as_slice()),
        (1003, b"O:1:\"X\":0:{}".as_slice()),
    ] {
        expected_stage.push(PostMetaRow {
            meta_id: id,
            post_id: 1,
            meta_key: Some(b"bounded_public_fixture".to_vec()),
            meta_value: Some(raw.to_vec()),
        });
    }

    // Only fixed public dummy values are inserted by this test controller.
    // Existing rows, option selection/autoload policy and production readers
    // are unchanged. A returned error still proceeds through deletion/restore.
    let exercised: ContractResult = async {
        for (id, raw) in [
            (1001u64, b"N;".as_slice()),
            (1002, b"b:0;".as_slice()),
            (1003, b"O:1:\"X\":0:{}".as_slice()),
        ] {
            sqlx::query(
                "INSERT INTO wp_postmeta (meta_id,post_id,meta_key,meta_value) VALUES (?,?,?,?)",
            )
            .bind(id)
            .bind(1u64)
            .bind(b"bounded_public_fixture".as_slice())
            .bind(raw)
            .execute(&mut *control)
            .await?;
        }
        let mut snapshot = repository.snapshot().await?;
        let observed: ContractResult = async {
            let raw_meta = snapshot.post_meta_page(None, page).await?;
            let raw_options = snapshot.options_page(None, page).await?;
            require(raw_meta == expected_stage && raw_options == original_options)?;
            // Missing record, SQL NULL and a decoded serialized Null remain
            // three different controller states. SQL NULL is not fed to WP.
            let select = |id| {
                raw_meta
                    .iter()
                    .find(|row| row.meta_id == id)
                    .map(|row| row.meta_value.as_deref())
            };
            require(select(9999).is_none())?;
            require(select(1) == Some(None))?;
            require(select(2) == Some(Some(b"".as_slice())))?;
            require(
                decode(
                    select(2)
                        .flatten()
                        .ok_or_else(|| std::io::Error::other("missing public fixture"))?,
                    UnserializeLimits::new(0, 0, 0, 0)?,
                )? == PhpValue::Bytes(Vec::new()),
            )?;
            require(
                decode(
                    select(1001)
                        .flatten()
                        .ok_or_else(|| std::io::Error::other("missing public fixture"))?,
                    UnserializeLimits::new(2, 0, 0, 0)?,
                )? == PhpValue::Null,
            )?;
            require(
                decode(
                    select(1002)
                        .flatten()
                        .ok_or_else(|| std::io::Error::other("missing public fixture"))?,
                    UnserializeLimits::new(4, 0, 0, 0)?,
                )? == PhpValue::Bool(false),
            )?;
            require(
                decode(
                    select(1003)
                        .flatten()
                        .ok_or_else(|| std::io::Error::other("missing public fixture"))?,
                    UnserializeLimits::new(32, 1, 32, 1)?,
                ) == Err(LimitedUnserializeError::UnsupportedValue),
            )?;

            let serialized = b"a:1:{s:3:\"key\";s:3:\"a\0b\";}";
            let exact = UnserializeLimits::new(serialized.len(), 1, 6, 1)?;
            // No first/last duplicate-meta preference is inferred. Explicit
            // raw row IDs 3 and 4 retain their separate values and order.
            let duplicates: Vec<_> = raw_meta
                .iter()
                .filter(|row| row.meta_key.as_deref() == Some(b"duplicate".as_slice()))
                .collect();
            require(
                duplicates.len() == 2 && duplicates[0].meta_id == 3 && duplicates[1].meta_id == 4,
            )?;
            require(select(3) == Some(Some(serialized.as_slice())))?;
            let decoded = decode(serialized, exact)?;
            match &decoded {
                PhpValue::Array(array) => require(
                    array.len() == 1
                        && array.get(&PhpKey::from_bytes(b"key"))
                            == Some(&PhpValue::Bytes(b"a\0b".to_vec())),
                )?,
                _ => require(false)?,
            }
            require(
                decode(
                    select(4)
                        .flatten()
                        .ok_or_else(|| std::io::Error::other("missing public fixture"))?,
                    UnserializeLimits::new(6, 0, 6, 0)?,
                )? == PhpValue::Bytes(b"second".to_vec()),
            )?;
            // The caller explicitly selects these option IDs; autoload values
            // remain raw and do not choose which options to decode.
            for id in [1, 2, 3] {
                let option = raw_options
                    .iter()
                    .find(|row| row.option_id == id)
                    .ok_or_else(|| std::io::Error::other("missing public fixture"))?;
                require(
                    option.option_value.as_slice() == serialized
                        && decode(&option.option_value, exact)? == decoded,
                )?;
            }
            for (limits, error) in [
                (
                    UnserializeLimits::new(serialized.len() - 1, 1, 6, 1)?,
                    LimitedUnserializeError::InputLimit,
                ),
                (
                    UnserializeLimits::new(serialized.len(), 0, 6, 1)?,
                    LimitedUnserializeError::EntryLimit,
                ),
                (
                    UnserializeLimits::new(serialized.len(), 1, 5, 1)?,
                    LimitedUnserializeError::OwnedBytesLimit,
                ),
                (
                    UnserializeLimits::new(serialized.len(), 1, 6, 0)?,
                    LimitedUnserializeError::DepthLimit,
                ),
            ] {
                require(decode(serialized, limits) == Err(error))?;
            }
            require(snapshot.post_meta_page(None, page).await? == raw_meta)?;
            require(snapshot.options_page(None, page).await? == raw_options)?;
            Ok(())
        }
        .await;
        let finished = snapshot.finish().await;
        // Also examine a fresh read-only snapshot before fixture cleanup.
        let unchanged: ContractResult = async {
            let mut fresh = repository.snapshot().await?;
            let meta = fresh.post_meta_page(None, page).await?;
            let options = fresh.options_page(None, page).await?;
            fresh.finish().await?;
            require(meta == expected_stage && options == original_options)?;
            Ok(())
        }
        .await;
        observed?;
        finished?;
        unchanged?;
        Ok(())
    }
    .await;
    let cleanup = sqlx::query("DELETE FROM wp_postmeta WHERE meta_id IN (1001,1002,1003)")
        .execute(&mut *control)
        .await;
    let restored: ContractResult = async {
        let mut after = repository.snapshot().await?;
        let meta = after.post_meta_page(None, page).await?;
        let options = after.options_page(None, page).await?;
        after.finish().await?;
        require(meta == original_meta && options == original_options)
    }
    .await;
    exercised?;
    cleanup?;
    restored?;
    Ok(())
}

async fn post_name_preprocess_lookup_pipeline(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    use recastpress_wp::{
        SanitizeTitleContext::{Query, Save},
        SanitizeTitleError, sanitize_title_utf8_default_callback_explicit_locale_bytes as sanitize,
    };
    let limit = NonZeroU16::new(32).unwrap();
    // This call precedes meta_view_database_tests: five initial meta rows plus
    // ID0 are six, before that later contract adds another five.
    let mut before = repository.snapshot().await?;
    let original_posts = before.posts_page(None, limit).await?;
    let original_meta = before.post_meta_page(None, limit).await?;
    before.finish().await?;
    assert_eq!((original_posts.len(), original_meta.len()), (4, 6));
    assert!(
        original_posts
            .iter()
            .all(|row| !(1001..=1002).contains(&row.id))
    );

    let raw = "Ä Name / Part".as_bytes();
    let fallback = b"RAW\0<unapproved>";
    let explicit_type = "bridge";
    for (id, name, status) in [
        (1001u64, "%C3%84-NAME-PART", "private"),
        (1002, std::str::from_utf8(fallback)?, "plugin_review"),
    ] {
        sqlx::query("INSERT INTO wp_posts (ID,post_author,post_name,post_type,post_status,post_content,post_title,post_excerpt,to_ping,pinged,post_content_filtered,post_password,post_parent,guid,menu_order,comment_count) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(id).bind(u64::MAX).bind(name).bind(explicit_type).bind(status)
            .bind("bridge本文\0🦀").bind("bridge raw title").bind("bridge excerpt")
            .bind("raw to_ping").bind("raw pinged").bind("raw filtered")
            .bind("private bridge dummy").bind(u64::MAX).bind("raw bridge guid")
            .bind(-23i32).bind(-11i64).execute(&mut *control).await?;
    }
    let mut stage = repository.snapshot().await?;
    let stage_posts = stage.posts_page(None, limit).await?;
    let stage_meta = stage.post_meta_page(None, limit).await?;
    assert_eq!((stage_posts.len(), stage_meta.len()), (6, 6));
    let query_bytes = sanitize(raw, b"", Query, b"en_US", usize::MAX)?;
    let save_bytes = sanitize(raw, b"", Save, b"en_US", usize::MAX)?;
    assert_eq!(query_bytes, b"%c3%84-name-part");
    assert_eq!(save_bytes, b"a-name-part");
    let fallback_bytes = sanitize(b"!", fallback, Query, b"en_US", usize::MAX)?;
    assert_eq!(fallback_bytes, fallback);
    let expected_query = stage_posts.iter().find(|row| row.id == 1001).unwrap();
    let expected_fallback = stage_posts.iter().find(|row| row.id == 1002).unwrap();
    // UTF-8 admission is an explicit host step, without lossy repair or a URL
    // policy. Both prepared name and caller-selected type remain text binds.
    let query_key = std::str::from_utf8(&query_bytes)?;
    let query_rows = stage
        .posts_by_name_text_page(query_key, explicit_type, None, limit)
        .await?;
    assert_eq!(query_rows, std::slice::from_ref(expected_query));
    assert_eq!(query_rows[0].post_name, "%C3%84-NAME-PART");
    assert_eq!(query_rows[0].post_status, "private");
    assert!(
        stage
            .posts_by_name_text_page(std::str::from_utf8(raw)?, explicit_type, None, limit)
            .await?
            .is_empty()
    );
    assert!(
        stage
            .posts_by_name_text_page(
                std::str::from_utf8(&save_bytes)?,
                explicit_type,
                None,
                limit
            )
            .await?
            .is_empty()
    );
    let fallback_key = std::str::from_utf8(&fallback_bytes)?;
    let fallback_rows = stage
        .posts_by_name_text_page(fallback_key, explicit_type, None, limit)
        .await?;
    assert_eq!(fallback_rows, std::slice::from_ref(expected_fallback));
    assert_eq!(fallback_rows[0].post_name.as_bytes(), fallback);
    assert_eq!(fallback_rows[0].post_status, "plugin_review");
    assert_eq!(fallback_rows[0].post_content, "bridge本文\0🦀");
    assert_eq!(fallback_rows[0].post_parent, u64::MAX);
    assert_eq!(fallback_rows[0].menu_order, -23);
    assert_eq!(fallback_rows[0].comment_count, -11);

    let arbitrary_bytes = b"\xff\0<unapproved>";
    let invalid_key = sanitize(b"!", arbitrary_bytes, Query, b"en_US", usize::MAX)?;
    assert_eq!(invalid_key, arbitrary_bytes);
    assert!(std::str::from_utf8(&invalid_key).is_err());
    // No lookup is called for invalid_key: there is no from_utf8_lossy path.
    for invalid_raw in [b"<".as_slice(), b"\xff", b"\xc3\0\x84"] {
        assert_eq!(
            sanitize(invalid_raw, fallback, Query, b"en_US", usize::MAX),
            Err(SanitizeTitleError::UnsupportedValue)
        );
    }
    assert_eq!(stage.posts_page(None, limit).await?, stage_posts);
    assert_eq!(stage.post_meta_page(None, limit).await?, stage_meta);
    stage.finish().await?;
    // A fresh snapshot also proves that the read/preprocess stage committed
    // no hidden row mutation; this is stronger than re-reading the same RR view.
    let mut fresh = repository.snapshot().await?;
    assert_eq!(fresh.posts_page(None, limit).await?, stage_posts);
    assert_eq!(fresh.post_meta_page(None, limit).await?, stage_meta);
    fresh.finish().await?;
    sqlx::query("DELETE FROM wp_posts WHERE ID BETWEEN 1001 AND 1002")
        .execute(&mut *control)
        .await?;
    let mut restored = repository.snapshot().await?;
    assert_eq!(restored.posts_page(None, limit).await?, original_posts);
    assert_eq!(restored.post_meta_page(None, limit).await?, original_meta);
    restored.finish().await?;
    Ok(())
}

async fn post_name_lookup_contract(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let limit = NonZeroU16::new(32).unwrap();
    let single = NonZeroU16::new(1).unwrap();
    let mut before = repository.snapshot().await?;
    let originals = before.posts_page(None, limit).await?;
    let original_meta = before.post_meta_page(None, limit).await?;
    before.finish().await?;
    assert_eq!((originals.len(), original_meta.len()), (4, 6));
    assert!(originals.iter().all(|row| !(1001..=1004).contains(&row.id)));
    for (id, name, post_type, status) in [
        (1001u64, "SlugÁlpha", "Póst", "private"),
        (1002, "SLUGALPHA ", "POST", "draft"),
        (1003, "quote'%\0名", "t'%\0", "plugin_review"),
        (1004, "", "", "publish"),
    ] {
        sqlx::query("INSERT INTO wp_posts (ID,post_author,post_name,post_type,post_status,post_content,post_title,post_excerpt,to_ping,pinged,post_content_filtered,post_password,post_parent,guid,menu_order,comment_count) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(id).bind(u64::MAX).bind(name).bind(post_type).bind(status)
            .bind("本文\0🦀").bind("private lookup title").bind("raw excerpt")
            .bind("raw to_ping").bind("raw pinged").bind("raw filtered")
            .bind("private lookup password").bind(u64::MAX).bind("raw guid")
            .bind(-17i32).bind(-9i64).execute(&mut *control).await?;
    }
    let mut snapshot = repository.snapshot().await?;
    let all = snapshot.posts_page(None, limit).await?;
    assert_eq!(all.len(), 8);
    // Independent text-bound SQL controls the column collation's equality;
    // raw full-row equality is checked against the established page reader.
    for (name, post_type) in [
        ("SlugÁlpha", "Póst"),
        ("slugalpha", "post"),
        ("SLUGÁLPHA   ", "PÓST   "),
        ("quote'%\0名", "t'%\0"),
        ("quote'%名", "t'%"),
        ("", ""),
        ("", "post"),
        ("missing", "post"),
        ("' OR 1=1 --", "post"),
        ("SlugÁlpha", "' OR 1=1 --"),
        (" slugalpha", "post"),
        ("SlugÁlpha", " post"),
        ("日本語🦀", "post"),
    ] {
        let ids: Vec<(u64,)> = sqlx::query_as(
            "SELECT ID FROM wp_posts WHERE post_name=? AND post_type=? ORDER BY ID ASC",
        )
        .bind(name)
        .bind(post_type)
        .fetch_all(&mut *control)
        .await?;
        let rows = snapshot
            .posts_by_name_text_page(name, post_type, None, limit)
            .await?;
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            ids.iter().map(|row| row.0).collect::<Vec<_>>()
        );
        for row in rows {
            assert_eq!(
                all.iter().find(|expected| expected.id == row.id),
                Some(&row)
            );
        }
    }
    let same = snapshot
        .posts_by_name_text_page("slugalpha", "post", None, limit)
        .await?;
    assert_eq!(
        same.iter().map(|row| row.id).collect::<Vec<_>>(),
        [1001, 1002]
    );
    assert_eq!(same[0].post_status, "private");
    assert_eq!(same[1].post_status, "draft");
    assert_eq!(same[0].post_name, "SlugÁlpha");
    assert_eq!(same[0].post_type, "Póst");
    assert_eq!(same[0].post_content, "本文\0🦀");
    assert_eq!(same[0].post_date, "0000-00-00 00:00:00");
    assert_eq!(same[0].post_author, u64::MAX);
    assert_eq!(same[0].comment_count, -9);
    assert_eq!(
        snapshot
            .posts_by_name_text_page("quote'%\0名", "t'%\0", None, limit)
            .await?[0]
            .id,
        1003
    );
    assert_eq!(
        snapshot
            .posts_by_name_text_page("", "", None, limit)
            .await?[0]
            .id,
        1004
    );
    assert!(
        snapshot
            .posts_by_name_text_page(&"x".repeat(201), &"y".repeat(21), None, limit)
            .await?
            .is_empty()
    );

    let mut cursor = None;
    let mut paged = Vec::new();
    loop {
        let rows = snapshot
            .posts_by_name_text_page("slugalpha", "post", cursor, single)
            .await?;
        if rows.is_empty() {
            break;
        }
        assert_eq!(rows.len(), 1);
        cursor = Some(rows[0].id);
        paged.extend(rows);
    }
    assert_eq!(paged, same);
    // Original stored ID0/MAX are queried using their actual raw keys. Neither
    // the absent cursor nor the maximum boundary is a signed/sentinel value.
    for id in [0, u64::MAX] {
        let row = originals.iter().find(|row| row.id == id).unwrap();
        let matches = snapshot
            .posts_by_name_text_page(&row.post_name, &row.post_type, None, limit)
            .await?;
        assert!(matches.contains(row));
        if id == 0 {
            assert_eq!(matches[0].id, 0);
            assert!(
                snapshot
                    .posts_by_name_text_page(&row.post_name, &row.post_type, Some(0), limit)
                    .await?
                    .iter()
                    .all(|row| row.id > 0)
            );
        }
        assert!(
            snapshot
                .posts_by_name_text_page(&row.post_name, &row.post_type, Some(u64::MAX), limit)
                .await?
                .is_empty()
        );
    }
    sqlx::query("UPDATE wp_posts SET post_name='replacement-lookup',post_title='replacement-title' WHERE ID=1002")
        .execute(&mut *control).await?;
    assert_eq!(
        snapshot
            .posts_by_name_text_page("slugalpha", "post", None, limit)
            .await?,
        same
    );
    assert!(
        snapshot
            .posts_by_name_text_page("replacement-lookup", "post", None, limit)
            .await?
            .is_empty()
    );
    let refused = sqlx::query("UPDATE wp_posts SET post_title='forbidden' WHERE ID=1001")
        .execute(&mut *snapshot.transaction)
        .await;
    let code = match refused {
        Err(sqlx::Error::Database(error)) => error
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .map(sqlx::mysql::MySqlDatabaseError::number),
        _ => None,
    };
    assert_eq!(code, Some(1792));
    snapshot.finish().await?;
    let mut fresh = repository.snapshot().await?;
    assert_eq!(
        fresh
            .posts_by_name_text_page("slugalpha", "post", None, limit)
            .await?
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [1001]
    );
    let changed = fresh
        .posts_by_name_text_page("replacement-lookup", "post", None, limit)
        .await?;
    let mut expected = all.iter().find(|row| row.id == 1002).unwrap().clone();
    expected.post_name = "replacement-lookup".into();
    expected.post_title = "replacement-title".into();
    assert_eq!(changed, [expected]);
    sqlx::query("SET NAMES latin1")
        .execute(&mut *fresh.transaction)
        .await?;
    assert_eq!(
        fresh
            .posts_by_name_text_page("SECRET", "SECRET", None, limit)
            .await,
        Err(PostNameLookupError::ProfileMismatch)
    );
    sqlx::query("SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci")
        .execute(&mut *fresh.transaction)
        .await?;
    fresh.finish().await?;

    for (prefix, change) in [
        (
            "post_name_engine_",
            "ALTER TABLE post_name_engine_posts ENGINE=MyISAM",
        ),
        (
            "post_name_binary_",
            "ALTER TABLE post_name_binary_posts MODIFY post_name varchar(200) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT ''",
        ),
        (
            "post_name_width_",
            "ALTER TABLE post_name_width_posts MODIFY post_name varchar(199) NOT NULL DEFAULT ''",
        ),
        (
            "post_type_binary_",
            "ALTER TABLE post_type_binary_posts MODIFY post_type varchar(20) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT 'post'",
        ),
        (
            "post_type_width_",
            "ALTER TABLE post_type_width_posts MODIFY post_type varchar(19) NOT NULL DEFAULT 'post'",
        ),
    ] {
        let name = format!("{prefix}posts");
        QueryBuilder::<MySql>::new("CREATE TABLE `")
            .push(&name)
            .push("` LIKE wp_posts")
            .build()
            .execute(&mut *control)
            .await?;
        sqlx::query(change).execute(&mut *control).await?;
        let mut rejected = repository.snapshot().await?;
        rejected.prefix = TablePrefix::new(prefix)?;
        assert_eq!(
            rejected
                .posts_by_name_text_page("SECRET", "SECRET", None, limit)
                .await,
            Err(PostNameLookupError::ProfileMismatch)
        );
        rejected.finish().await?;
        QueryBuilder::<MySql>::new("DROP TABLE `")
            .push(&name)
            .push("`")
            .build()
            .execute(&mut *control)
            .await?;
    }
    let mut missing = repository.snapshot().await?;
    missing.prefix = TablePrefix::new("post_name_missing_")?;
    assert_eq!(
        missing
            .posts_by_name_text_page("SECRET", "SECRET", None, limit)
            .await,
        Err(PostNameLookupError::ProfileMismatch)
    );
    missing.finish().await?;
    // Query and decode failures remain different closed classes.
    sqlx::query("CREATE TABLE post_name_incomplete_posts (post_name varchar(200),post_type varchar(20)) ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci").execute(&mut *control).await?;
    let mut incomplete = repository.snapshot().await?;
    incomplete.prefix = TablePrefix::new("post_name_incomplete_")?;
    assert_eq!(
        incomplete
            .posts_by_name_text_page("SECRET", "SECRET", None, limit)
            .await,
        Err(PostNameLookupError::QueryFailure)
    );
    incomplete.finish().await?;
    sqlx::query("DROP TABLE post_name_incomplete_posts")
        .execute(&mut *control)
        .await?;
    sqlx::query("CREATE TABLE post_name_decode_posts LIKE wp_posts")
        .execute(&mut *control)
        .await?;
    sqlx::query("ALTER TABLE post_name_decode_posts MODIFY post_title blob NOT NULL")
        .execute(&mut *control)
        .await?;
    sqlx::query("INSERT INTO post_name_decode_posts SELECT * FROM wp_posts WHERE ID=1001")
        .execute(&mut *control)
        .await?;
    sqlx::query("UPDATE post_name_decode_posts SET post_title=? WHERE ID=1001")
        .bind(vec![0xffu8])
        .execute(&mut *control)
        .await?;
    let mut decode = repository.snapshot().await?;
    decode.prefix = TablePrefix::new("post_name_decode_")?;
    assert_eq!(
        decode
            .posts_by_name_text_page("slugalpha", "post", None, limit)
            .await,
        Err(PostNameLookupError::RowDecodeFailure)
    );
    decode.finish().await?;
    sqlx::query("DROP TABLE post_name_decode_posts")
        .execute(&mut *control)
        .await?;
    sqlx::query("DELETE FROM wp_posts WHERE ID BETWEEN 1001 AND 1004")
        .execute(&mut *control)
        .await?;
    let mut after = repository.snapshot().await?;
    assert_eq!(after.posts_page(None, limit).await?, originals);
    assert_eq!(after.post_meta_page(None, limit).await?, original_meta);
    after.finish().await?;
    Ok(())
}

async fn user_lookup_contract(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let limit = NonZeroU16::new(32).unwrap();
    let mut before = repository.snapshot().await?;
    let originals = before.users_page(None, limit).await?;
    let meta = before.user_meta_page(None, limit).await?;
    before.finish().await?;
    assert!(originals.iter().all(|row| !(1001..=1009).contains(&row.id)));
    let input = [
        (1001u64, "LóginAlpha", "first@lookup.invalid"),
        (1002, "日本語🦀", "unicode@lookup.invalid"),
        (1003, "nul\0login", "nul\0@lookup.invalid"),
        (1004, "", ""),
        (1005, "0", "0"),
        (1006, "email@lookup.invalid", "second@lookup.invalid"),
        (1007, "other-lookup", "EMAIL@lookup.invalid"),
        (1008, "query' OR 1=1 --", "query@lookup.invalid"),
        (1009, "duplicate-lookup", "FIRST@lookup.invalid"),
    ];
    for (id, login, email) in input {
        sqlx::query("INSERT INTO wp_users (ID,user_login,user_pass,user_nicename,user_email,user_url,user_activation_key,user_status,display_name) VALUES (?,?,'private-lookup-pass','lookup',?,'','private-lookup-activation',-7,'private-lookup-display')")
            .bind(id).bind(login).bind(email).execute(&mut *control).await?;
    }
    let mut snapshot = repository.snapshot().await?;
    for key in ["LóginAlpha", "loginalpha", "LOGINÁLPHA", "loginalpha   "] {
        let found = snapshot.user_by_login_text_query(key).await?.unwrap();
        assert_eq!(found.id, 1001);
        assert_eq!(found.user_login, "LóginAlpha".as_bytes());
        assert_eq!(found.user_status, -7); // retained raw, never an auth decision
        assert_eq!(found.user_pass, b"private-lookup-pass");
        assert_eq!(found.user_activation_key, b"private-lookup-activation");
        assert_eq!(format!("{found:?}"), "UserRow { id: 1001, .. }");
    }
    for (key, expected) in [
        ("日本語🦀", 1002),
        ("nul\0login", 1003),
        ("", 1004),
        ("0", 1005),
        ("query' OR 1=1 --", 1008),
    ] {
        assert_eq!(
            snapshot.user_by_login_text_query(key).await?.unwrap().id,
            expected
        );
    }
    for key in [
        "absent-lookup",
        "' OR 1=1 --",
        " loginalpha",
        "nul",
        &"x".repeat(61),
    ] {
        assert!(snapshot.user_by_login_text_query(key).await?.is_none());
    }
    // Fixed unicode_ci compares the NUL as ignorable here; preserving returned
    // bytes is different from imposing binary equality in the WHERE clause.
    for key in ["nul\0@lookup.invalid", "nul@lookup.invalid"] {
        let (control_id, control_email): (u64, Vec<u8>) = sqlx::query_as(
            "SELECT ID,CAST(user_email AS BINARY) FROM wp_users WHERE user_email=? LIMIT 1",
        )
        .bind(key)
        .fetch_one(&mut *control)
        .await?;
        assert_eq!(control_id, 1003);
        assert_eq!(control_email, b"nul\0@lookup.invalid");
        let found = snapshot.user_by_email_text_query(key).await?.unwrap();
        assert_eq!(found.id, control_id);
        assert_eq!(found.user_email, control_email);
    }
    assert!(
        snapshot
            .user_by_email_text_query(&"x".repeat(101))
            .await?
            .is_none()
    );
    // SQL lookup does not choose username-versus-email priority for its caller.
    assert_eq!(
        snapshot
            .user_by_login_text_query("email@lookup.invalid")
            .await?
            .unwrap()
            .id,
        1006
    );
    assert_eq!(
        snapshot
            .user_by_email_text_query("email@lookup.invalid")
            .await?
            .unwrap()
            .id,
        1007
    );
    let duplicate = snapshot
        .user_by_email_text_query("first@lookup.invalid   ")
        .await?
        .unwrap();
    assert!([1001, 1009].contains(&duplicate.id)); // no ORDER BY or unique-email promise
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wp_users WHERE user_email=?")
        .bind("first@lookup.invalid")
        .fetch_one(&mut *control)
        .await?;
    assert_eq!(count, 2);
    sqlx::query("UPDATE wp_users SET user_login='replacement-lookup',user_pass='replacement-pass' WHERE ID=1001")
        .execute(&mut *control).await?;
    assert_eq!(
        snapshot
            .user_by_login_text_query("loginalpha")
            .await?
            .unwrap()
            .user_pass,
        b"private-lookup-pass"
    );
    assert!(
        snapshot
            .user_by_login_text_query("replacement-lookup")
            .await?
            .is_none()
    );
    let refused = sqlx::query("UPDATE wp_users SET user_pass='forbidden' WHERE ID=1001")
        .execute(&mut *snapshot.transaction)
        .await;
    let code = match refused {
        Err(sqlx::Error::Database(error)) => error
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .map(sqlx::mysql::MySqlDatabaseError::number),
        _ => None,
    };
    assert_eq!(code, Some(1792));
    snapshot.finish().await?;
    let mut fresh = repository.snapshot().await?;
    assert!(
        fresh
            .user_by_login_text_query("loginalpha")
            .await?
            .is_none()
    );
    assert_eq!(
        fresh
            .user_by_login_text_query("replacement-lookup")
            .await?
            .unwrap()
            .user_pass,
        b"replacement-pass"
    );
    // Connection profile rejection does not expose a key or driver diagnostic.
    sqlx::query("SET NAMES latin1")
        .execute(&mut *fresh.transaction)
        .await?;
    assert_eq!(
        fresh.user_by_login_text_query("private-lookup-key").await,
        Err(UserLookupError::ProfileMismatch)
    );
    sqlx::query("SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci")
        .execute(&mut *fresh.transaction)
        .await?;
    fresh.finish().await?;
    for (prefix, change) in [
        (
            "lookup_nontransactional_",
            "ALTER TABLE lookup_nontransactional_users ENGINE=MyISAM",
        ),
        (
            "lookup_binary_",
            "ALTER TABLE lookup_binary_users MODIFY user_login varchar(60) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT ''",
        ),
        (
            "lookup_width_",
            "ALTER TABLE lookup_width_users MODIFY user_email varchar(99) NOT NULL DEFAULT ''",
        ),
    ] {
        let name = format!("{prefix}users");
        QueryBuilder::<MySql>::new("CREATE TABLE `")
            .push(&name)
            .push("` LIKE wp_users")
            .build()
            .execute(&mut *control)
            .await?;
        sqlx::query(change).execute(&mut *control).await?;
        let mut profile = repository.snapshot().await?;
        profile.prefix = TablePrefix::new(prefix)?;
        assert_eq!(
            profile.user_by_login_text_query("private-lookup-key").await,
            Err(UserLookupError::ProfileMismatch)
        );
        profile.finish().await?;
        QueryBuilder::<MySql>::new("DROP TABLE `")
            .push(&name)
            .push("`")
            .build()
            .execute(&mut *control)
            .await?;
    }
    sqlx::query("CREATE TABLE lookup_incomplete_users (user_login varchar(60),user_email varchar(100)) ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci")
        .execute(&mut *control).await?;
    let mut incomplete = repository.snapshot().await?;
    incomplete.prefix = TablePrefix::new("lookup_incomplete_")?;
    assert_eq!(
        incomplete
            .user_by_email_text_query("private-lookup-key")
            .await,
        Err(UserLookupError::QueryFailure)
    );
    incomplete.finish().await?;
    sqlx::query("DROP TABLE lookup_incomplete_users")
        .execute(&mut *control)
        .await?;
    sqlx::query("DELETE FROM wp_users WHERE ID BETWEEN 1001 AND 1009")
        .execute(&mut *control)
        .await?;
    let mut after = repository.snapshot().await?;
    assert_eq!(after.users_page(None, limit).await?, originals);
    assert_eq!(after.user_meta_page(None, limit).await?, meta);
    after.finish().await?;
    Ok(())
}

async fn drive_user_credential_stage<'a>(
    snapshot: &mut EditingSnapshot,
    prepared: recastpress_wp::PreparedLoginInput<'a>,
    limits: recastpress_wp::HostPasswordWorkLimits,
) -> Result<(recastpress_wp::CredentialStageOutcome<UserRow>, [usize; 2]), Box<dyn std::error::Error>>
{
    use recastpress_wp::{CredentialLookup, CredentialStage};
    fn stored_hash(row: &UserRow) -> &[u8] {
        &row.user_pass
    }
    let mut calls = [0, 0];
    let mut stage = CredentialStage::begin(prepared, limits, stored_hash);
    loop {
        stage = match stage {
            CredentialStage::NeedLogin(request) => {
                assert_eq!(calls[0], 0, "owned stage must not repeat login lookup");
                calls[0] += 1;
                let reply = match snapshot
                    .user_by_login_text_query(std::str::from_utf8(request.query_key())?)
                    .await
                {
                    Ok(Some(row)) => CredentialLookup::Found(row),
                    Ok(None) => CredentialLookup::NotFound,
                    Err(_) => CredentialLookup::LookupUnresolved,
                };
                request.respond(reply)
            }
            CredentialStage::NeedEmail(request) => {
                assert_eq!(calls[1], 0, "owned stage must not repeat email lookup");
                calls[1] += 1;
                let reply = match snapshot
                    .user_by_email_text_query(std::str::from_utf8(request.query_key())?)
                    .await
                {
                    Ok(Some(row)) => CredentialLookup::Found(row),
                    Ok(None) => CredentialLookup::NotFound,
                    Err(_) => CredentialLookup::LookupUnresolved,
                };
                request.respond(reply)
            }
            CredentialStage::Complete(outcome) => return Ok((outcome, calls)),
        };
    }
}

async fn credential_username_email_stage_pipeline(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
    options: MySqlConnectOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use recastpress_wp::{
        CredentialLookupKind, CredentialStageOutcome, CredentialUnresolvedReason,
        HostMaxBcryptCost, HostMaxPhpassCost, HostPasswordWorkLimits, PreparedLoginInput,
        RehashIntent, WpPasswordHashOutcome, wp_check_password_raw_empty_filters_bytes as verify,
        wp_hash_password_default_bcrypt_bytes as generate,
        wp_password_needs_rehash_default_bcrypt_bytes as needs_rehash,
    };
    const PASSWORD: &[u8] = b"public-owned-rehash-dummy";
    const CORRECT: &[u8] = b"1353bb1343967c24b73dd0e089c998ae";
    const WRONG: &[u8] = b"d41d8cd98f00b204e9800998ecf8427e";
    const RAW_PASSWORD: &[u8] = b"\0 \tpublic-owned-rehash-dummy\r\n\0";
    const RAW_USERNAME: &[u8] = b" \tshared@example.test\x0b";
    const LOCALE: &[u8] = b"en_US";
    let limit = NonZeroU16::new(32).unwrap();
    let mut before = repository.snapshot().await?;
    let originals = before.users_page(None, limit).await?;
    let meta = before.user_meta_page(None, limit).await?;
    assert_eq!(originals.len(), 3);
    assert_eq!(meta.len(), 5);
    assert!(originals.iter().all(|row| row.id != 1001 && row.id != 1002));
    before.finish().await?;
    for (id, login, email, hash) in [
        (1001u64, "shared@example.test", "other@example.test", WRONG),
        (1002, "email-other", "shared@example.test", CORRECT),
    ] {
        sqlx::query("INSERT INTO wp_users (ID,user_login,user_pass,user_email,user_activation_key) VALUES (?,?,?,?, 'public-stage-activation')")
            .bind(id).bind(login).bind(hash).bind(email).execute(&mut *control).await?;
    }
    let maximum = HostMaxBcryptCost::new(12)?;
    let limits = HostPasswordWorkLimits::new(maximum, HostMaxPhpassCost::new(13)?);
    let untouched = fresh_user(repository, 1001).await?;
    let prepared = PreparedLoginInput::new(RAW_USERNAME, RAW_PASSWORD, LOCALE, 1024)?;
    assert_eq!(prepared.username_stage1(), b"shared@example.test");
    assert_eq!(prepared.password_trimmed(), PASSWORD);
    let mut snapshot = repository.snapshot().await?;
    let before_stage = snapshot.users_page(None, limit).await?;
    let (result, calls) = drive_user_credential_stage(&mut snapshot, prepared, limits).await?;
    assert_eq!(calls, [1, 1]);
    let CredentialStageOutcome::Match(candidate) = result else {
        panic!("owned username mismatch must reach the email candidate");
    };
    assert!(matches!(
        candidate.matched_via(),
        CredentialLookupKind::Email
    ));
    assert!(matches!(candidate.rehash_intent(), RehashIntent::Suggested));
    assert_eq!(candidate.record().id, 1002);
    assert_eq!(candidate.record().user_pass, CORRECT);
    assert_eq!(snapshot.users_page(None, limit).await?, before_stage);
    snapshot.finish().await?;
    // Only this explicit outer controller generates and writes. The stage's
    // opaque raw candidate is not a principal, session or write capability.
    let WpPasswordHashOutcome::Hash(hash) = generate(PASSWORD, maximum)? else {
        panic!("bounded public dummy must produce the typed hash outcome");
    };
    let replacement = UserCredentialFields {
        user_pass: hash.as_wp_storage_bytes().to_vec(),
        user_activation_key: Vec::new(),
    };
    let writer = EditingCredentialWriter::connect(options, TablePrefix::new("wp_")?).await?;
    assert_eq!(
        writer
            .compare_and_set_user_credentials(candidate.record(), &replacement)
            .await?,
        UserCredentialWriteOutcome::Updated
    );
    let current = fresh_user(repository, 1002).await?;
    assert_eq!(
        current,
        with_user_credentials(candidate.record(), &replacement)
    );
    assert_eq!(verify(PASSWORD, &current.user_pass, limits), Ok(true));
    assert_eq!(needs_rehash(&current.user_pass), Ok(false));
    assert_eq!(fresh_user(repository, 1001).await?, untouched);
    writer.close().await;
    for (stored, maximum, reason) in [
        (CORRECT, 12, None),
        (
            b"unsupported-public-stage-hash-with-more-than-32-bytes".as_slice(),
            12,
            Some(CredentialUnresolvedReason::UnsupportedHash),
        ),
        (
            b"$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C".as_slice(),
            4,
            Some(CredentialUnresolvedReason::BcryptCostLimit),
        ),
    ] {
        sqlx::query("UPDATE wp_users SET user_pass=? WHERE ID=1001")
            .bind(stored)
            .execute(&mut *control)
            .await?;
        let limits = HostPasswordWorkLimits::new(
            HostMaxBcryptCost::new(maximum)?,
            HostMaxPhpassCost::new(13)?,
        );
        let prepared = PreparedLoginInput::new(RAW_USERNAME, RAW_PASSWORD, LOCALE, 1024)?;
        let mut snapshot = repository.snapshot().await?;
        let baseline = snapshot.users_page(None, limit).await?;
        let (outcome, calls) = drive_user_credential_stage(&mut snapshot, prepared, limits).await?;
        assert_eq!(calls, [1, 0]);
        if let Some(expected) = reason {
            assert!(
                matches!(outcome,CredentialStageOutcome::Unresolved{reason,..} if reason==expected)
            );
        } else {
            let CredentialStageOutcome::Match(candidate) = outcome else {
                panic!("definite username match must stop before email");
            };
            assert!(matches!(
                candidate.matched_via(),
                CredentialLookupKind::Login
            ));
            assert_eq!(candidate.record().id, 1001);
        }
        assert_eq!(snapshot.users_page(None, limit).await?, baseline);
        snapshot.finish().await?;
        let mut fresh = repository.snapshot().await?;
        assert_eq!(fresh.users_page(None, limit).await?, baseline);
        fresh.finish().await?;
    }
    for (username, password) in [
        (b"".as_slice(), PASSWORD),
        (b"0", PASSWORD),
        (b"&x;0", PASSWORD),
        (b"shared@example.test", b"0".as_slice()),
    ] {
        let prepared = PreparedLoginInput::new(username, password, LOCALE, 1024)?;
        assert!(prepared.username_php_empty() || prepared.password_php_empty());
        let mut snapshot = repository.snapshot().await?;
        let baseline = snapshot.users_page(None, limit).await?;
        let (outcome, calls) = drive_user_credential_stage(&mut snapshot, prepared, limits).await?;
        assert_eq!(calls, [0, 0]);
        assert!(matches!(outcome, CredentialStageOutcome::NoCredentialMatch));
        assert_eq!(snapshot.users_page(None, limit).await?, baseline);
        snapshot.finish().await?;
    }
    sqlx::query("DELETE FROM wp_users WHERE ID IN (1001,1002)")
        .execute(&mut *control)
        .await?;
    let mut after = repository.snapshot().await?;
    assert_eq!(after.users_page(None, limit).await?, originals);
    assert_eq!(after.user_meta_page(None, limit).await?, meta);
    after.finish().await?;
    Ok(())
}

async fn credential_preprocess_lookup_pipeline(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
    options: MySqlConnectOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use recastpress_wp::{
        HostMaxBcryptCost, HostMaxPhpassCost, HostPasswordWorkLimits, LoginLookupPlan,
        PreparedLoginInput, WpPasswordHashOutcome,
        wp_check_password_raw_empty_filters_bytes as verify,
        wp_hash_password_default_bcrypt_bytes as generate,
        wp_password_needs_rehash_default_bcrypt_bytes as needs_rehash,
    };
    const ID: u64 = 1001;
    const PASSWORD: &[u8] = b"public-owned-rehash-dummy";
    const OLD_HASH: &[u8] = b"1353bb1343967c24b73dd0e089c998ae";
    const RAW_USERNAME: &[u8] = b" \tL\xc3\xb3GIN%20&dummy;\x0b";
    const RAW_PASSWORD: &[u8] = b"\0 \tpublic-owned-rehash-dummy\r\n\0";
    const LOCALE: &[u8] = b"en_US";
    // Prefix-state checks are pure and never become principal/auth decisions.
    let non_idempotent = PreparedLoginInput::new(b"%&x;61", PASSWORD, LOCALE, 1024)?;
    assert_eq!(non_idempotent.username_stage1(), b"%61");
    assert!(!non_idempotent.username_php_empty());
    let LoginLookupPlan::QueryKey(empty_key) = non_idempotent.login_lookup_plan()? else {
        panic!("nonempty S1 reaches second sanitize even when S2 becomes empty");
    };
    assert!(empty_key.is_empty());
    let ff = PreparedLoginInput::new(b"\x0c", PASSWORD, LOCALE, 1024)?;
    assert_eq!(ff.username_stage1(), b" ");
    assert!(!ff.username_php_empty());
    assert!(matches!(ff.login_lookup_plan()?, LoginLookupPlan::Skipped));
    let username_zero = PreparedLoginInput::new(b"&x;0", PASSWORD, LOCALE, 1024)?;
    assert_eq!(username_zero.username_stage1(), b"0");
    assert!(username_zero.username_php_empty());
    assert!(matches!(
        username_zero.login_lookup_plan()?,
        LoginLookupPlan::Skipped
    ));
    let password_zero = PreparedLoginInput::new(b"login", b"\0 0\t", LOCALE, 1024)?;
    assert_eq!(password_zero.password_trimmed(), b"0");
    assert!(password_zero.password_php_empty());
    let limit = NonZeroU16::new(32).unwrap();
    let mut before = repository.snapshot().await?;
    let originals = before.users_page(None, limit).await?;
    let meta = before.user_meta_page(None, limit).await?;
    assert_eq!(originals.len(), 3);
    assert_eq!(meta.len(), 5);
    assert!(originals.iter().all(|row| row.id != ID));
    before.finish().await?;
    sqlx::query("INSERT INTO wp_users (ID,user_login,user_pass,user_email,user_activation_key) VALUES (1001,'login',?,'owned-preprocess@example.invalid','public-owned-activation')")
        .bind(OLD_HASH).execute(&mut *control).await?;
    let prepared = PreparedLoginInput::new(RAW_USERNAME, RAW_PASSWORD, LOCALE, 1024)?;
    assert_eq!(prepared.username_stage1(), b"LoGIN");
    assert_eq!(prepared.email_lookup_key(), b"LoGIN");
    assert_eq!(prepared.password_trimmed(), PASSWORD);
    assert!(!prepared.username_php_empty());
    assert!(!prepared.password_php_empty());
    let LoginLookupPlan::QueryKey(key) = prepared.login_lookup_plan()? else {
        panic!("public owned nonempty prefix must produce the raw SQL query plan");
    };
    assert_eq!(key, b"LoGIN");
    let text_key = std::str::from_utf8(&key)?;
    let mut old_snapshot = repository.snapshot().await?;
    assert!(
        old_snapshot
            .user_by_login_text_query(std::str::from_utf8(RAW_USERNAME)?)
            .await?
            .is_none()
    );
    let old = old_snapshot
        .user_by_login_text_query(text_key)
        .await?
        .expect("fixed collation finds the explicitly owned lowercase login");
    assert_eq!(old.id, ID);
    assert_eq!(old.user_login, b"login");
    assert_eq!(old.user_pass, OLD_HASH);
    let maximum = HostMaxBcryptCost::new(12)?;
    let work = HostPasswordWorkLimits::new(maximum, HostMaxPhpassCost::new(13)?);
    assert_eq!(verify(RAW_PASSWORD, &old.user_pass, work), Ok(false));
    assert_eq!(
        verify(prepared.password_trimmed(), &old.user_pass, work),
        Ok(true)
    );
    assert_eq!(needs_rehash(&old.user_pass), Ok(true));
    let WpPasswordHashOutcome::Hash(hash) = generate(prepared.password_trimmed(), maximum)? else {
        panic!("bounded public input requires the typed secure hash outcome");
    };
    let replacement = UserCredentialFields {
        user_pass: hash.as_wp_storage_bytes().to_vec(),
        user_activation_key: Vec::new(),
    };
    let writer = EditingCredentialWriter::connect(options, TablePrefix::new("wp_")?).await?;
    assert_eq!(
        writer
            .compare_and_set_user_credentials(&old, &replacement)
            .await?,
        UserCredentialWriteOutcome::Updated
    );
    let mut fresh = repository.snapshot().await?;
    let current = fresh
        .user_by_login_text_query(text_key)
        .await?
        .expect("owned lookup survives the credential pair update");
    assert_eq!(current, with_user_credentials(&old, &replacement));
    assert_eq!(
        verify(prepared.password_trimmed(), &current.user_pass, work),
        Ok(true)
    );
    assert_eq!(needs_rehash(&current.user_pass), Ok(false));
    fresh.finish().await?;
    old_snapshot.finish().await?;
    writer.close().await;
    sqlx::query("DELETE FROM wp_users WHERE ID=1001")
        .execute(&mut *control)
        .await?;
    let mut after = repository.snapshot().await?;
    assert_eq!(after.users_page(None, limit).await?, originals);
    assert_eq!(after.user_meta_page(None, limit).await?, meta);
    after.finish().await?;
    Ok(())
}

async fn credential_rehash_pipeline(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
    options: MySqlConnectOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use recastpress_wp::{
        HostMaxBcryptCost, HostMaxPhpassCost, HostPasswordWorkLimits, WpPasswordHashError,
        WpPasswordHashOutcome, wp_check_password_raw_empty_filters_bytes as verify,
        wp_hash_password_default_bcrypt_bytes as generate,
        wp_password_needs_rehash_default_bcrypt_bytes as needs_rehash,
    };
    const ID: u64 = 1001;
    const PASSWORD: &[u8] = b"public-owned-rehash-dummy";
    const OLD_HASH: &[u8] = b"1353bb1343967c24b73dd0e089c998ae";
    let limit = NonZeroU16::new(32).unwrap();
    let mut before = repository.snapshot().await?;
    let originals = before.users_page(None, limit).await?;
    let meta = before.user_meta_page(None, limit).await?;
    assert_eq!(originals.len(), 3);
    assert_eq!(meta.len(), 5);
    assert!(originals.iter().all(|row| row.id != ID));
    before.finish().await?;
    // Explicit test-controller selection. This finite owned row is never a
    // general login, automatic rehash, session or HTTP authorization policy.
    sqlx::query("INSERT INTO wp_users (ID,user_login,user_pass,user_email,user_activation_key) VALUES (1001,'owned-rehash-fixture',?,'owned-rehash@example.invalid','public-owned-activation')")
        .bind(OLD_HASH).execute(&mut *control).await?;
    let mut old_snapshot = repository.snapshot().await?;
    let old = old_snapshot
        .users_page(None, limit)
        .await?
        .into_iter()
        .find(|row| row.id == ID)
        .expect("explicit owned credential row exists");
    assert_eq!(old.user_pass, OLD_HASH);
    let maximum = HostMaxBcryptCost::new(12)?;
    let work = HostPasswordWorkLimits::new(maximum, HostMaxPhpassCost::new(13)?);
    assert_eq!(verify(PASSWORD, &old.user_pass, work), Ok(true));
    assert_eq!(needs_rehash(&old.user_pass), Ok(true));
    assert!(matches!(
        generate(PASSWORD, HostMaxBcryptCost::new(4)?),
        Err(WpPasswordHashError::CostLimit)
    ));
    assert_eq!(fresh_user(repository, ID).await?, old);
    let WpPasswordHashOutcome::Hash(secure_hash) = generate(PASSWORD, maximum)? else {
        panic!("bounded public dummy must produce the typed hash outcome");
    };
    // The caller explicitly selects storage bytes and an empty activation key;
    // the DB writer does not choose generation or key-clearing policy.
    let replacement = UserCredentialFields {
        user_pass: secure_hash.as_wp_storage_bytes().to_vec(),
        user_activation_key: Vec::new(),
    };
    let writer = EditingCredentialWriter::connect(options, TablePrefix::new("wp_")?).await?;
    assert_eq!(
        writer
            .compare_and_set_user_credentials(&old, &replacement)
            .await?,
        UserCredentialWriteOutcome::Updated
    );
    assert_eq!(
        old_snapshot
            .users_page(None, limit)
            .await?
            .into_iter()
            .find(|row| row.id == ID),
        Some(old.clone())
    );
    let current = fresh_user(repository, ID).await?;
    assert_eq!(current, with_user_credentials(&old, &replacement));
    assert_eq!(verify(PASSWORD, &current.user_pass, work), Ok(true));
    assert_eq!(
        verify(b"wrong-public-owned-rehash-dummy", &current.user_pass, work),
        Ok(false)
    );
    assert_eq!(needs_rehash(&current.user_pass), Ok(false));
    assert_eq!(
        writer
            .compare_and_set_user_credentials(&old, &replacement)
            .await?,
        UserCredentialWriteOutcome::Conflict
    );
    assert_eq!(fresh_user(repository, ID).await?, current);
    old_snapshot.finish().await?;
    writer.close().await;
    sqlx::query("DELETE FROM wp_users WHERE ID=1001")
        .execute(&mut *control)
        .await?;
    let mut after = repository.snapshot().await?;
    assert_eq!(after.users_page(None, limit).await?, originals);
    assert_eq!(after.user_meta_page(None, limit).await?, meta);
    after.finish().await?;
    Ok(())
}

async fn fresh_user(repository: &EditingRepository, id: u64) -> Result<UserRow, sqlx::Error> {
    let mut read = repository.snapshot().await?;
    let row = read
        .users_page(None, NonZeroU16::new(32).unwrap())
        .await?
        .into_iter()
        .find(|row| row.id == id)
        .expect("owned fixture user exists");
    read.finish().await?;
    Ok(row)
}
fn user_credentials(row: &UserRow) -> UserCredentialFields {
    UserCredentialFields {
        user_pass: row.user_pass.clone(),
        user_activation_key: row.user_activation_key.clone(),
    }
}
fn with_user_credentials(row: &UserRow, fields: &UserCredentialFields) -> UserRow {
    let mut row = row.clone();
    row.user_pass = fields.user_pass.clone();
    row.user_activation_key = fields.user_activation_key.clone();
    row
}

async fn user_credential_writer_contract(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
    options: MySqlConnectOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use UserCredentialWriteOutcome::*;
    let writer =
        EditingCredentialWriter::connect(options.clone(), TablePrefix::new("wp_")?).await?;
    let other = EditingCredentialWriter::connect(options.clone(), TablePrefix::new("wp_")?).await?;
    let limit = NonZeroU16::new(32).unwrap();
    let mut before = repository.snapshot().await?;
    let originals = before.users_page(None, limit).await?;
    let meta = before.user_meta_page(None, limit).await?;
    before.finish().await?;
    assert_eq!(originals.len(), 3);
    // Public fixed dummy storage bytes only; no WP generator dependency,
    // verification, login, rehash, session or credential policy is invoked.
    let replacement = UserCredentialFields {
        user_pass: b"$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C".to_vec(),
        user_activation_key: b"new\0activation-' OR 1=1 --".to_vec(),
    };
    for id in [0, u64::MAX, 1] {
        let old = fresh_user(repository, id).await?;
        if id == 0 || id == u64::MAX {
            assert_eq!(old.user_registered, "0000-00-00 00:00:00");
        }
        let mut stale = repository.snapshot().await?;
        assert_eq!(
            stale
                .users_page(None, limit)
                .await?
                .into_iter()
                .find(|row| row.id == id),
            Some(old.clone())
        );
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&old, &replacement)
                .await?,
            Updated
        );
        assert_eq!(
            stale
                .users_page(None, limit)
                .await?
                .into_iter()
                .find(|row| row.id == id),
            Some(old.clone())
        );
        let refused = sqlx::query("UPDATE wp_users SET user_pass='must not write' WHERE ID=?")
            .bind(id)
            .execute(&mut *stale.transaction)
            .await;
        let code = match refused {
            Err(sqlx::Error::Database(error)) => error
                .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                .map(sqlx::mysql::MySqlDatabaseError::number),
            _ => None,
        };
        assert_eq!(code, Some(1792));
        stale.finish().await?;
        let current = fresh_user(repository, id).await?;
        assert_eq!(current, with_user_credentials(&old, &replacement));
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&current, &replacement)
                .await?,
            NoChange
        );
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&old, &replacement)
                .await?,
            Conflict
        );
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&current, &user_credentials(&old))
                .await?,
            Updated
        );
        assert_eq!(fresh_user(repository, id).await?, old);
    }
    let original = fresh_user(repository, 1).await?;
    let mut absent = original.clone();
    absent.id = 99;
    assert_eq!(
        writer
            .compare_and_set_user_credentials(&absent, &user_credentials(&original))
            .await?,
        NotFound
    );
    // Even collation-equal differences in a non-credential field must conflict
    // before the NoChange check. No SQL collation/rows-affected CAS token.
    for nicename in ["case-sensitive", "Cáse-Sensitive", "Case-Sensitive "] {
        let (count,): (u64,) = sqlx::query_as(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wp_users WHERE ID=1 AND user_nicename=?",
        )
        .bind(nicename)
        .fetch_one(&mut *control)
        .await?;
        assert_eq!(count, 1);
        let mut stale = original.clone();
        stale.user_nicename = nicename.as_bytes().to_vec();
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&stale, &user_credentials(&original))
                .await?,
            Conflict
        );
        assert_eq!(fresh_user(repository, 1).await?, original);
    }
    for changed in 0..8 {
        let mut stale = original.clone();
        match changed {
            0 => stale.user_login.push(b'x'),
            1 => stale.user_nicename.push(b'x'),
            2 => stale.user_email.push(b'x'),
            3 => stale.user_url.push(b'x'),
            4 => stale.user_registered = "2021-01-01 00:00:00".into(),
            5 => stale.user_status = original.user_status.wrapping_add(1),
            6 => stale.display_name.push(b'x'),
            // ID is the selector; an unused ID returns NotFound, never creates.
            _ => stale.id = 99,
        }
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&stale, &replacement)
                .await?,
            if changed == 7 { NotFound } else { Conflict }
        );
    }
    // Both directions exercise a partial UPDATE followed by full rollback:
    // one varchar(255) truncates while the other field would otherwise change.
    for too_long in [
        UserCredentialFields {
            user_pass: vec![b'x'; 256],
            user_activation_key: b"must-rollback-key".to_vec(),
        },
        UserCredentialFields {
            user_pass: b"must-rollback-pass".to_vec(),
            user_activation_key: vec![b'x'; 256],
        },
    ] {
        assert_eq!(
            writer
                .compare_and_set_user_credentials(&original, &too_long)
                .await,
            Err(UserCredentialWriteError::StorageMismatch)
        );
        assert_eq!(fresh_user(repository, 1).await?, original);
    }
    let a = UserCredentialFields {
        user_pass: b"public-dummy-race-A".to_vec(),
        user_activation_key: b"public-dummy-key-A".to_vec(),
    };
    let b = UserCredentialFields {
        user_pass: b"public-dummy-race-B".to_vec(),
        user_activation_key: b"public-dummy-key-B".to_vec(),
    };
    let (first, second) = tokio::join!(
        writer.compare_and_set_user_credentials(&original, &a),
        other.compare_and_set_user_credentials(&original, &b)
    );
    let outcomes = [first?, second?];
    assert_eq!(
        outcomes.iter().filter(|value| **value == Updated).count(),
        1
    );
    assert_eq!(
        outcomes.iter().filter(|value| **value == Conflict).count(),
        1
    );
    let current = fresh_user(repository, 1).await?;
    assert_eq!(
        current,
        with_user_credentials(&original, if outcomes[0] == Updated { &a } else { &b })
    );
    assert_eq!(
        writer
            .compare_and_set_user_credentials(&current, &user_credentials(&original))
            .await?,
        Updated
    );
    sqlx::query("CREATE TABLE rp_credential_reject_users LIKE wp_users")
        .execute(&mut *control)
        .await?;
    sqlx::query("INSERT INTO rp_credential_reject_users SELECT * FROM wp_users WHERE ID=1")
        .execute(&mut *control)
        .await?;
    sqlx::query("ALTER TABLE rp_credential_reject_users ENGINE=MyISAM")
        .execute(&mut *control)
        .await?;
    let rejected = EditingCredentialWriter::connect(
        options.clone(),
        TablePrefix::new("rp_credential_reject_")?,
    )
    .await?;
    assert_eq!(
        rejected
            .compare_and_set_user_credentials(&original, &a)
            .await,
        Err(UserCredentialWriteError::Database)
    );
    let (pass,key):(Vec<u8>,Vec<u8>)=sqlx::query_as("SELECT CAST(user_pass AS BINARY),CAST(user_activation_key AS BINARY) FROM rp_credential_reject_users WHERE ID=1")
        .fetch_one(&mut *control).await?;
    assert_eq!(pass, original.user_pass);
    assert_eq!(key, original.user_activation_key);
    rejected.close().await;
    sqlx::query("DROP TABLE rp_credential_reject_users")
        .execute(&mut *control)
        .await?;
    let missing =
        EditingCredentialWriter::connect(options, TablePrefix::new("missing_credential_fixture_")?)
            .await?;
    assert_eq!(
        missing
            .compare_and_set_user_credentials(&original, &a)
            .await,
        Err(UserCredentialWriteError::Database)
    );
    missing.close().await;
    let mut after = repository.snapshot().await?;
    assert_eq!(after.users_page(None, limit).await?, originals);
    assert_eq!(after.user_meta_page(None, limit).await?, meta);
    after.finish().await?;
    other.close().await;
    writer.close().await;
    Ok(())
}

async fn fresh_post(repository: &EditingRepository, id: u64) -> Result<PostRow, sqlx::Error> {
    let mut read = repository.snapshot().await?;
    let row = read.post(id).await?.expect("owned fixture post exists");
    read.finish().await?;
    Ok(row)
}
fn post_text(row: &PostRow) -> PostTextFields {
    PostTextFields {
        title: row.post_title.clone(),
        content: row.post_content.clone(),
        excerpt: row.post_excerpt.clone(),
    }
}
fn with_post_text(row: &PostRow, text: &PostTextFields) -> PostRow {
    let mut row = row.clone();
    row.post_title = text.title.clone();
    row.post_content = text.content.clone();
    row.post_excerpt = text.excerpt.clone();
    row
}

async fn post_text_writer_contract(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
    options: MySqlConnectOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use PostTextWriteOutcome::*;
    let writer = EditingWriter::connect(options.clone(), TablePrefix::new("wp_")?).await?;
    let other = EditingWriter::connect(options.clone(), TablePrefix::new("wp_")?).await?;
    let mut originals = Vec::new();
    for id in [0, u64::MAX, 1] {
        let old = fresh_post(repository, id).await?;
        originals.push(old.clone());
        assert_eq!(old.post_date, "0000-00-00 00:00:00");
        assert_eq!(old.post_modified, "0000-00-00 00:00:00");
        let mut stale_snapshot = repository.snapshot().await?;
        assert_eq!(stale_snapshot.post(id).await?, Some(old.clone()));
        let text = PostTextFields {
            title: format!("CAS {id} 日本語 ' OR 1=1 --"),
            content: "new\0body 🦀".into(),
            excerpt: "new excerpt & <text>".into(),
        };
        assert_eq!(
            writer.compare_and_set_post_text(&old, &text).await?,
            Updated
        );
        assert_eq!(stale_snapshot.post(id).await?, Some(old.clone()));
        stale_snapshot.finish().await?;
        let current = fresh_post(repository, id).await?;
        assert_eq!(current, with_post_text(&old, &text));
        assert_eq!(
            writer.compare_and_set_post_text(&current, &text).await?,
            NoChange
        );
        assert_eq!(
            writer.compare_and_set_post_text(&old, &text).await?,
            Conflict
        );
        assert_eq!(
            writer
                .compare_and_set_post_text(&current, &post_text(&old))
                .await?,
            Updated
        );
        assert_eq!(fresh_post(repository, id).await?, old);
    }
    let original = fresh_post(repository, 1).await?;
    let mut absent = original.clone();
    absent.id = 99;
    assert_eq!(
        writer
            .compare_and_set_post_text(&absent, &post_text(&original))
            .await?,
        NotFound
    );
    let mut missing = repository.snapshot().await?;
    assert!(missing.post(99).await?.is_none());
    missing.finish().await?;
    let collated = PostTextFields {
        title: "Café ".into(),
        content: original.post_content.clone(),
        excerpt: original.post_excerpt.clone(),
    };
    assert_eq!(
        writer
            .compare_and_set_post_text(&original, &collated)
            .await?,
        Updated
    );
    let current = fresh_post(repository, 1).await?;
    for title in ["café ", "Cafe ", "Café"] {
        let (same,): (u64,) = sqlx::query_as(
            "SELECT CAST(COUNT(*) AS UNSIGNED) FROM wp_posts WHERE ID=1 AND post_title=?",
        )
        .bind(title)
        .fetch_one(&mut *control)
        .await?;
        assert_eq!(same, 1, "fixture collation must equate the selected values");
        let mut expected = current.clone();
        expected.post_title = title.into();
        assert_eq!(
            writer
                .compare_and_set_post_text(&expected, &post_text(&current))
                .await?,
            Conflict
        );
        assert_eq!(fresh_post(repository, 1).await?, current);
    }
    assert_eq!(
        writer
            .compare_and_set_post_text(&current, &post_text(&original))
            .await?,
        Updated
    );
    let too_long = PostTextFields {
        title: "x".repeat(65536),
        content: "must roll back this content".into(),
        excerpt: "must roll back excerpt".into(),
    };
    assert_eq!(
        writer.compare_and_set_post_text(&original, &too_long).await,
        Err(PostTextWriteError::StorageMismatch)
    );
    assert_eq!(fresh_post(repository, 1).await?, original);
    let a = PostTextFields {
        title: "race A".into(),
        content: "race body A".into(),
        excerpt: "race excerpt A".into(),
    };
    let b = PostTextFields {
        title: "race B".into(),
        content: "race body B".into(),
        excerpt: "race excerpt B".into(),
    };
    let (a_result, b_result) = tokio::join!(
        writer.compare_and_set_post_text(&original, &a),
        other.compare_and_set_post_text(&original, &b)
    );
    let outcomes = [a_result?, b_result?];
    assert_eq!(
        outcomes.iter().filter(|result| **result == Updated).count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Conflict)
            .count(),
        1
    );
    let current = fresh_post(repository, 1).await?;
    assert_eq!(
        current,
        with_post_text(&original, if outcomes[0] == Updated { &a } else { &b })
    );
    assert_eq!(
        writer
            .compare_and_set_post_text(&current, &post_text(&original))
            .await?,
        Updated
    );
    sqlx::query("CREATE TABLE rp_writer_reject_posts LIKE wp_posts")
        .execute(&mut *control)
        .await?;
    sqlx::query("INSERT INTO rp_writer_reject_posts SELECT * FROM wp_posts WHERE ID=1")
        .execute(&mut *control)
        .await?;
    sqlx::query("ALTER TABLE rp_writer_reject_posts ENGINE=MyISAM")
        .execute(&mut *control)
        .await?;
    let nontransactional =
        EditingWriter::connect(options.clone(), TablePrefix::new("rp_writer_reject_")?).await?;
    let rejected = nontransactional
        .compare_and_set_post_text(&original, &a)
        .await;
    let mut read_rejected = QueryBuilder::<MySql>::new("SELECT ");
    read_rejected
        .push(POST_FIELDS)
        .push(" FROM rp_writer_reject_posts WHERE ID=1");
    let stored = PostRow::decode(read_rejected.build().fetch_one(&mut *control).await?)?;
    nontransactional.close().await;
    sqlx::query("DROP TABLE rp_writer_reject_posts")
        .execute(&mut *control)
        .await?;
    assert_eq!(rejected, Err(PostTextWriteError::Database));
    assert_eq!(stored, original);
    assert_eq!(fresh_post(repository, 1).await?, original);
    let missing_writer =
        EditingWriter::connect(options, TablePrefix::new("missing_writer_fixture_")?).await?;
    assert_eq!(
        missing_writer
            .compare_and_set_post_text(&original, &a)
            .await,
        Err(PostTextWriteError::Database)
    );
    missing_writer.close().await;
    for old in originals {
        assert_eq!(fresh_post(repository, old.id).await?, old);
    }
    other.close().await;
    writer.close().await;
    Ok(())
}

async fn alpha_post_inputs(
    repository: &EditingRepository,
    control: &mut MySqlConnection,
    options: MySqlConnectOptions,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    assert_eq!(
        std::env::var("RECASTPRESS_DB_FIXTURE").as_deref(),
        Ok("disposable")
    );
    assert_eq!(
        std::env::var("RECASTPRESS_ALPHA_POST_PATH").as_deref(),
        Ok("disposable")
    );
    const ID: u64 = 1001;
    let mut snapshot = repository.snapshot().await?;
    assert!(snapshot.post(ID).await?.is_none());
    let originals = snapshot
        .posts_page(None, NonZeroU16::new(16).unwrap())
        .await?;
    let meta = snapshot
        .post_meta_page(None, NonZeroU16::new(32).unwrap())
        .await?;
    snapshot.finish().await?;
    assert_eq!(originals.len(), 4);
    assert_eq!(meta.len(), 11);
    assert!(
        originals
            .iter()
            .all(|post| post.post_status != "publish" || !post.post_password.is_empty())
    );
    // Explicit test-controller selection, not status/slug/date-derived product
    // publication policy. Never select an old fixture article for export.
    sqlx::query("INSERT INTO wp_posts (ID,post_author,post_title,post_content,post_excerpt,to_ping,pinged,post_content_filtered,post_status,post_type,post_password,post_name) VALUES (?,0,'alpha seed','alpha seed body','','','','','publish','post','','alpha-owned-fixture')")
        .bind(ID).execute(&mut *control).await?;
    let writer = EditingWriter::connect(options, TablePrefix::new("wp_")?).await?;
    let mut current = fresh_post(repository, ID).await?;
    let mut inputs = serde_json::Map::new();
    for (key, title, content) in [
        (
            "a",
            "Alpha A & owned article",
            "Alpha A public body\n<literal>&\"' 日本語",
        ),
        (
            "b",
            "Alpha B & owned article",
            "Alpha B public body\n<literal>&\"' 日本語",
        ),
    ] {
        let text = PostTextFields {
            title: title.into(),
            content: content.into(),
            excerpt: "alpha fixture excerpt".into(),
        };
        assert_eq!(
            writer.compare_and_set_post_text(&current, &text).await?,
            PostTextWriteOutcome::Updated
        );
        current = fresh_post(repository, ID).await?;
        assert_eq!(current.id, ID);
        assert_eq!(current.post_status, "publish");
        assert_eq!(current.post_type, "post");
        assert!(current.post_password.is_empty());
        assert_eq!(current.post_title, title);
        assert_eq!(current.post_content, content);
        let exported = [current.post_title.as_str(), current.post_content.as_str()];
        for old in &originals {
            for secret in [&old.post_title, &old.post_content, &old.post_password] {
                if !secret.is_empty() {
                    assert!(
                        exported
                            .iter()
                            .all(|value| !value.contains(secret.as_str()))
                    );
                }
            }
            assert!(
                exported
                    .iter()
                    .all(|value| **value != old.post_author.to_string())
            );
        }
        for row in &meta {
            for value in [&row.meta_key, &row.meta_value].into_iter().flatten() {
                if !value.is_empty() {
                    assert!(exported.iter().all(|text| {
                        !text
                            .as_bytes()
                            .windows(value.len())
                            .any(|part| part == value)
                    }));
                }
            }
        }
        inputs.insert(key.into(),serde_json::json!({"format_version":1,"posts":[{"target":"/alpha-post","title":current.post_title,"body_text":current.post_content}]}));
    }
    writer.close().await;
    sqlx::query("DELETE FROM wp_posts WHERE ID=?")
        .bind(ID)
        .execute(&mut *control)
        .await?;
    let mut after = repository.snapshot().await?;
    assert_eq!(
        after.posts_page(None, NonZeroU16::new(16).unwrap()).await?,
        originals
    );
    assert_eq!(
        after
            .post_meta_page(None, NonZeroU16::new(32).unwrap())
            .await?,
        meta
    );
    after.finish().await?;
    Ok(serde_json::Value::Object(inputs))
}

async fn taxonomy_contract(
    repository: &EditingRepository,
    writer: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = "非公開の分類\0 🦀 ' OR 1=1 --".as_bytes();
    for (id, group) in [(0u64, i64::MIN), (1, -1), (u64::MAX, i64::MAX)] {
        sqlx::query("INSERT INTO wp_terms (term_id,name,slug,term_group) VALUES (?,?,?,?)")
            .bind(id)
            .bind(name)
            .bind(b"Case-Sensitive\0slug".as_slice())
            .bind(group)
            .execute(&mut *writer)
            .await?;
    }
    for (id, term, count) in [
        (0u64, 0u64, i64::MIN),
        (1, 1, -1),
        (u64::MAX, 999, i64::MAX),
    ] {
        sqlx::query("INSERT INTO wp_term_taxonomy (term_taxonomy_id,term_id,taxonomy,description,parent,count) VALUES (?,?,?,?,?,?)")
            .bind(id).bind(term).bind(b"plugin_Taxonomy".as_slice()).bind(name).bind(u64::MAX).bind(count).execute(&mut *writer).await?;
    }
    let pairs = [
        (0u64, 0u64),
        (0, 1),
        (0, u64::MAX),
        (1, 0),
        (1, 1),
        (1, u64::MAX),
        (u64::MAX, 0),
        (u64::MAX, u64::MAX),
    ];
    for (index, &(object, taxonomy)) in pairs.iter().enumerate() {
        sqlx::query("INSERT INTO wp_term_relationships VALUES (?,?,?)")
            .bind(object)
            .bind(taxonomy)
            .bind(if index % 2 == 0 { i32::MIN } else { i32::MAX })
            .execute(&mut *writer)
            .await?;
    }
    let serial = b"a:1:{s:1:\"k\";s:3:\"a\0b\";}";
    for (id, term, key, value) in [
        (0u64, 0u64, None, None),
        (1, 1, Some(b"".as_slice()), Some(b"".as_slice())),
        (2, 1, Some(b"duplicate".as_slice()), Some(serial.as_slice())),
        (
            3,
            1,
            Some(b"duplicate".as_slice()),
            Some(b"second".as_slice()),
        ),
        (u64::MAX, 999, Some(b"orphan".as_slice()), Some(name)),
    ] {
        sqlx::query(
            "INSERT INTO wp_termmeta (meta_id,term_id,meta_key,meta_value) VALUES (?,?,?,?)",
        )
        .bind(id)
        .bind(term)
        .bind(key)
        .bind(value)
        .execute(&mut *writer)
        .await?;
    }
    let ten = NonZeroU16::new(10).unwrap();
    let one = NonZeroU16::new(1).unwrap();
    let mut snapshot = repository.snapshot().await?;
    let terms = snapshot.terms_page(None, ten).await?;
    assert_eq!(
        terms.iter().map(|row| row.term_id).collect::<Vec<_>>(),
        [0, 1, u64::MAX]
    );
    assert!(
        terms
            .iter()
            .all(|row| row.name == name && row.slug == b"Case-Sensitive\0slug")
    );
    assert_eq!(
        terms.iter().map(|row| row.term_group).collect::<Vec<_>>(),
        [i64::MIN, -1, i64::MAX]
    );
    assert_eq!(snapshot.terms_page(Some(0), one).await?[0].term_id, 1);
    assert!(snapshot.terms_page(Some(u64::MAX), ten).await?.is_empty());
    let taxonomies = snapshot.term_taxonomy_page(None, ten).await?;
    assert_eq!(
        taxonomies
            .iter()
            .map(|row| row.term_taxonomy_id)
            .collect::<Vec<_>>(),
        [0, 1, u64::MAX]
    );
    assert!(
        taxonomies
            .iter()
            .all(|row| row.taxonomy == b"plugin_Taxonomy"
                && row.description == name
                && row.parent == u64::MAX)
    );
    assert_eq!(
        taxonomies.iter().map(|row| row.count).collect::<Vec<_>>(),
        [i64::MIN, -1, i64::MAX]
    );
    assert_eq!(taxonomies[2].term_id, 999); // No inner join dropping orphan references.
    assert_eq!(
        snapshot.term_taxonomy_page(Some(0), one).await?[0].term_taxonomy_id,
        1
    );
    assert!(
        snapshot
            .term_taxonomy_page(Some(u64::MAX), ten)
            .await?
            .is_empty()
    );
    let metadata = snapshot.term_meta_page(None, ten).await?;
    assert_eq!(metadata.len(), 5);
    assert_eq!(metadata[0].meta_id, 0);
    assert_eq!(
        (&metadata[0].meta_key, &metadata[0].meta_value),
        (&None, &None)
    );
    assert_eq!(
        (&metadata[1].meta_key, &metadata[1].meta_value),
        (&Some(vec![]), &Some(vec![]))
    );
    assert_eq!(metadata[2].meta_key, metadata[3].meta_key);
    assert_eq!(metadata[2].meta_value.as_deref(), Some(serial.as_slice()));
    assert_eq!(metadata[4].term_id, 999);
    assert_eq!(snapshot.term_meta_page(Some(0), one).await?[0].meta_id, 1);
    assert!(
        snapshot
            .term_meta_page(Some(u64::MAX), ten)
            .await?
            .is_empty()
    );
    let expected: Vec<_> = pairs
        .iter()
        .map(|&(object_id, term_taxonomy_id)| TermRelationshipKey {
            object_id,
            term_taxonomy_id,
        })
        .collect();
    // Page sizes split a single object's relationships at different boundaries.
    for size in [1, 2, 3] {
        let limit = NonZeroU16::new(size).unwrap();
        let mut after = None;
        let mut actual = Vec::new();
        loop {
            let page = snapshot.term_relationships_page(after, limit).await?;
            assert!(page.len() <= usize::from(size));
            if page.is_empty() {
                break;
            }
            for row in page {
                assert!(after.is_none_or(|key| row.key > key));
                assert_eq!(
                    row.term_order,
                    if actual.len() % 2 == 0 {
                        i32::MIN
                    } else {
                        i32::MAX
                    }
                );
                after = Some(row.key);
                actual.push(row.key);
                assert!(actual.len() <= expected.len(), "cursor did not advance");
            }
        }
        assert_eq!(actual, expected);
    }
    let first = TermRelationshipKey {
        object_id: 0,
        term_taxonomy_id: 0,
    };
    assert_eq!(
        snapshot.term_relationships_page(Some(first), one).await?[0].key,
        expected[1]
    );
    let boundary = TermRelationshipKey {
        object_id: 0,
        term_taxonomy_id: u64::MAX,
    };
    assert_eq!(
        snapshot
            .term_relationships_page(Some(boundary), one)
            .await?[0]
            .key,
        expected[3]
    );
    for sql in [
        "UPDATE wp_terms SET name='changed' WHERE term_id=1",
        "UPDATE wp_term_taxonomy SET description='changed' WHERE term_taxonomy_id=1",
        "UPDATE wp_term_relationships SET term_order=7 WHERE object_id=1 AND term_taxonomy_id=1",
        "UPDATE wp_termmeta SET meta_value='changed' WHERE meta_id=2",
    ] {
        sqlx::query(sql).execute(&mut *writer).await?;
    }
    assert_eq!(snapshot.terms_page(Some(0), one).await?[0], terms[1]);
    assert_eq!(
        snapshot.term_taxonomy_page(Some(0), one).await?[0],
        taxonomies[1]
    );
    assert_eq!(snapshot.term_meta_page(Some(1), one).await?[0], metadata[2]);
    assert_eq!(
        snapshot
            .term_relationships_page(Some(expected[3]), one)
            .await?[0]
            .term_order,
        i32::MIN
    );
    snapshot.finish().await?;
    let mut newer = repository.snapshot().await?;
    assert_eq!(newer.terms_page(Some(0), one).await?[0].name, b"changed");
    assert_eq!(
        newer.term_taxonomy_page(Some(0), one).await?[0].description,
        b"changed"
    );
    assert_eq!(
        newer.term_meta_page(Some(1), one).await?[0]
            .meta_value
            .as_deref(),
        Some(b"changed".as_slice())
    );
    assert_eq!(
        newer
            .term_relationships_page(Some(expected[3]), one)
            .await?[0]
            .term_order,
        7
    );
    newer.finish().await?;
    for table in [
        "wp_terms",
        "wp_term_taxonomy",
        "wp_term_relationships",
        "wp_termmeta",
    ] {
        // Names are fixed fixture literals, never request input.
        QueryBuilder::<MySql>::new("ALTER TABLE ")
            .push(table)
            .push(" ENGINE=MyISAM")
            .build()
            .execute(&mut *writer)
            .await?;
        match repository.snapshot().await {
            Err(sqlx::Error::Configuration(error)) => assert!(
                matches!(error.downcast_ref::<SnapshotSchemaError>(), Some(SnapshotSchemaError::NonTransactionalTable { table: name, .. }) if name == table)
            ),
            _ => panic!("taxonomy nontransactional table was not rejected"),
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
                matches!(error.downcast_ref::<SnapshotSchemaError>(), Some(SnapshotSchemaError::MissingTable { table: name }) if name == table)
            ),
            _ => panic!("taxonomy missing table was not rejected"),
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

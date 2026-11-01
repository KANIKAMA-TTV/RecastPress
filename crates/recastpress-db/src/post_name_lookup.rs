//! Editing-only caller-selected name/type text SQL pages, not a URL resolver.
use super::{EditingSnapshot, POST_FIELDS, PostRow, Table, TablePrefix};
use sqlx::{MySql, QueryBuilder};
use std::{fmt, num::NonZeroU16};

type ColumnProfileRow = (String, String, Option<u64>, Option<String>, Option<String>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostNameLookupError {
    ProfileMismatch,
    QueryFailure,
    RowDecodeFailure,
}
impl fmt::Display for PostNameLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProfileMismatch => {
                "post name lookup requires the fixed UTF-8 text/schema profile"
            }
            Self::QueryFailure => "post name lookup query failed",
            Self::RowDecodeFailure => "post name lookup row decoding failed",
        })
    }
}
impl std::error::Error for PostNameLookupError {}

fn query(
    prefix: &TablePrefix,
    name: &str,
    post_type: &str,
    after: Option<u64>,
    limit: NonZeroU16,
) -> QueryBuilder<MySql> {
    let mut query = QueryBuilder::new("SELECT ");
    query
        .push(POST_FIELDS)
        .push(" FROM ")
        .push(prefix.table(Table::Posts))
        .push(" WHERE post_name = ")
        .push_bind(name)
        .push(" AND post_type = ")
        .push_bind(post_type);
    if let Some(id) = after {
        query.push(" AND ID > ").push_bind(id);
    }
    query.push(" ORDER BY ID ASC LIMIT ").push_bind(limit.get());
    query
}

impl EditingSnapshot {
    async fn post_name_lookup_profile(&mut self) -> Result<(), PostNameLookupError> {
        let engine: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND CAST(TABLE_NAME AS BINARY)=CAST(? AS BINARY)",
        ).bind(self.prefix.raw_table(Table::Posts)).fetch_optional(&mut *self.transaction).await.map_err(|_| PostNameLookupError::QueryFailure)?;
        if !matches!(engine, Some((Some(engine),)) if engine.eq_ignore_ascii_case("InnoDB")) {
            return Err(PostNameLookupError::ProfileMismatch);
        }
        let connection: (String, String, String, String) = sqlx::query_as(
            "SELECT @@character_set_client,@@character_set_connection,@@character_set_results,@@collation_connection",
        ).fetch_one(&mut *self.transaction).await.map_err(|_| PostNameLookupError::QueryFailure)?;
        if connection
            != (
                "utf8mb4".into(),
                "utf8mb4".into(),
                "utf8mb4".into(),
                "utf8mb4_unicode_ci".into(),
            )
        {
            return Err(PostNameLookupError::ProfileMismatch);
        }
        // Explicit unsigned normalization avoids the MySQL/MariaDB metadata
        // signedness difference without changing the stored schema widths.
        let columns: Vec<ColumnProfileRow> = sqlx::query_as(
            "SELECT COLUMN_NAME,DATA_TYPE,CAST(CHARACTER_MAXIMUM_LENGTH AS UNSIGNED),CHARACTER_SET_NAME,COLLATION_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND CAST(TABLE_NAME AS BINARY)=CAST(? AS BINARY) AND COLUMN_NAME IN ('post_name','post_type') ORDER BY COLUMN_NAME",
        ).bind(self.prefix.raw_table(Table::Posts)).fetch_all(&mut *self.transaction).await.map_err(|_| PostNameLookupError::QueryFailure)?;
        if columns
            != vec![
                (
                    "post_name".into(),
                    "varchar".into(),
                    Some(200),
                    Some("utf8mb4".into()),
                    Some("utf8mb4_unicode_ci".into()),
                ),
                (
                    "post_type".into(),
                    "varchar".into(),
                    Some(20),
                    Some("utf8mb4".into()),
                    Some("utf8mb4_unicode_ci".into()),
                ),
            ]
        {
            return Err(PostNameLookupError::ProfileMismatch);
        }
        Ok(())
    }

    /// Both keys are bound as UTF-8 text, with the fixed column collation's
    /// equality. No trim/sanitize/length/status/parent/public/unique policy.
    /// None includes ID0; Some(id) means strictly after id, even at u64::MAX.
    /// Stable fixed schema only: metadata and SELECT are not an atomic DDL
    /// attestation. Returned fields retain the existing PostRow storage types.
    pub async fn posts_by_name_text_page(
        &mut self,
        name: &str,
        post_type: &str,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<PostRow>, PostNameLookupError> {
        self.post_name_lookup_profile().await?;
        query(&self.prefix, name, post_type, after, limit)
            .build()
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(|_| PostNameLookupError::QueryFailure)?
            .into_iter()
            .map(PostRow::decode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PostNameLookupError::RowDecodeFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn both_text_keys_and_cursor_limit_are_parameters_without_publication_filters() {
        let prefix = TablePrefix::new("wp_").unwrap();
        for after in [None, Some(0), Some(u64::MAX)] {
            let query = query(
                &prefix,
                "SECRET_NAME' OR 1=1 --\0",
                "SECRET_TYPE",
                after,
                NonZeroU16::new(1).unwrap(),
            );
            let sql = query.sql();
            let ending = if after.is_some() {
                " WHERE post_name = ? AND post_type = ? AND ID > ? ORDER BY ID ASC LIMIT ?"
            } else {
                " WHERE post_name = ? AND post_type = ? ORDER BY ID ASC LIMIT ?"
            };
            assert!(sql.as_str().ends_with(ending));
            assert!(!sql.as_str().contains("SECRET"));
            assert!(!sql.as_str().contains("WHERE CAST"));
            assert!(!sql.as_str().contains("post_status ="));
        }
    }

    #[test]
    fn errors_have_no_key_driver_payload_or_source() {
        for error in [
            PostNameLookupError::ProfileMismatch,
            PostNameLookupError::QueryFailure,
            PostNameLookupError::RowDecodeFailure,
        ] {
            assert!(error.source().is_none());
            assert!(!format!("{error:?}: {error}").contains("SECRET"));
        }
    }
}

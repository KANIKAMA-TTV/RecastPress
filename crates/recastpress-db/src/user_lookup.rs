//! Editing-only SQL subset of WP_User::get_data_by, not a login resolver.
//!
//! The caller supplies already selected UTF-8 text. No trim, sanitize, PHP
//! coercion/empty rule, cache, username/email priority, status or auth policy.
use super::{EditingSnapshot, Table, TablePrefix, UserRow};
use sqlx::{MySql, QueryBuilder, Row, mysql::MySqlRow};
use std::fmt;

/// Closed classifications do not retain a lookup key, row or driver error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserLookupError {
    ProfileMismatch,
    QueryFailure,
    RowDecodeFailure,
}
impl fmt::Display for UserLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProfileMismatch => "user lookup requires the fixed UTF-8 text/schema profile",
            Self::QueryFailure => "user lookup query failed",
            Self::RowDecodeFailure => "user lookup row decoding failed",
        })
    }
}
impl std::error::Error for UserLookupError {}

#[derive(Clone, Copy)]
enum Field {
    Login,
    Email,
}
impl Field {
    fn column(self) -> &'static str {
        match self {
            Self::Login => "user_login",
            Self::Email => "user_email",
        }
    }
}
// Binary casts preserve returned storage bytes only. WHERE remains a text
// column comparison; neither binary keys nor ORDER BY change WP's SQL subset.
const FIELDS: &str = "ID,CAST(user_login AS BINARY) AS user_login,CAST(user_pass AS BINARY) AS user_pass,CAST(user_nicename AS BINARY) AS user_nicename,CAST(user_email AS BINARY) AS user_email,CAST(user_url AS BINARY) AS user_url,CAST(user_registered AS CHAR) AS user_registered,CAST(user_activation_key AS BINARY) AS user_activation_key,user_status,CAST(display_name AS BINARY) AS display_name";

fn query(prefix: &TablePrefix, field: Field, key: &str) -> QueryBuilder<MySql> {
    let mut query = QueryBuilder::new("SELECT ");
    query
        .push(FIELDS)
        .push(" FROM ")
        .push(prefix.table(Table::Users));
    query
        .push(" WHERE ")
        .push(field.column())
        .push(" = ")
        .push_bind(key);
    query.push(" LIMIT 1");
    query
}

fn decode(row: MySqlRow) -> Result<UserRow, sqlx::Error> {
    Ok(UserRow {
        id: row.try_get("ID")?,
        user_login: row.try_get("user_login")?,
        user_pass: row.try_get("user_pass")?,
        user_nicename: row.try_get("user_nicename")?,
        user_email: row.try_get("user_email")?,
        user_url: row.try_get("user_url")?,
        user_registered: row.try_get("user_registered")?,
        user_activation_key: row.try_get("user_activation_key")?,
        user_status: row.try_get("user_status")?,
        display_name: row.try_get("display_name")?,
    })
}

impl EditingSnapshot {
    async fn user_lookup_profile(&mut self) -> Result<(), UserLookupError> {
        let engine: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND CAST(TABLE_NAME AS BINARY)=CAST(? AS BINARY)",
        ).bind(self.prefix.raw_table(Table::Users)).fetch_optional(&mut *self.transaction).await.map_err(|_| UserLookupError::QueryFailure)?;
        if !matches!(engine, Some((Some(engine),)) if engine.eq_ignore_ascii_case("InnoDB")) {
            return Err(UserLookupError::ProfileMismatch);
        }
        let connection: (String, String, String, String) = sqlx::query_as(
            "SELECT @@character_set_client,@@character_set_connection,@@character_set_results,@@collation_connection",
        ).fetch_one(&mut *self.transaction).await.map_err(|_| UserLookupError::QueryFailure)?;
        if connection
            != (
                "utf8mb4".into(),
                "utf8mb4".into(),
                "utf8mb4".into(),
                "utf8mb4_unicode_ci".into(),
            )
        {
            return Err(UserLookupError::ProfileMismatch);
        }
        // MySQL reports this nonnegative width as signed BIGINT while MariaDB
        // reports unsigned. Normalize the wire type, retaining widths and NULL.
        let columns: Vec<(String, String, Option<u64>, Option<String>)> = sqlx::query_as(
            "SELECT COLUMN_NAME,DATA_TYPE,CAST(CHARACTER_MAXIMUM_LENGTH AS UNSIGNED),COLLATION_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND CAST(TABLE_NAME AS BINARY)=CAST(? AS BINARY) AND COLUMN_NAME IN ('user_login','user_email') ORDER BY COLUMN_NAME",
        ).bind(self.prefix.raw_table(Table::Users)).fetch_all(&mut *self.transaction).await.map_err(|_| UserLookupError::QueryFailure)?;
        let expected = vec![
            (
                "user_email".into(),
                "varchar".into(),
                Some(100),
                Some("utf8mb4_unicode_ci".into()),
            ),
            (
                "user_login".into(),
                "varchar".into(),
                Some(60),
                Some("utf8mb4_unicode_ci".into()),
            ),
        ];
        if columns != expected {
            return Err(UserLookupError::ProfileMismatch);
        }
        Ok(())
    }

    async fn user_text_query(
        &mut self,
        field: Field,
        key: &str,
    ) -> Result<Option<UserRow>, UserLookupError> {
        self.user_lookup_profile().await?;
        query(&self.prefix, field, key)
            .build()
            .fetch_optional(&mut *self.transaction)
            .await
            .map_err(|_| UserLookupError::QueryFailure)?
            .map(decode)
            .transpose()
            .map_err(|_| UserLookupError::RowDecodeFailure)
    }

    /// Raw text SQL subset: WHERE user_login = ? LIMIT 1 in this READ ONLY
    /// snapshot. Caller preprocessing/cache/auth are not implemented. There is
    /// no source query-length guard; the stored varchar width is not an input
    /// limit. Stable fixed schema/utf8mb4_unicode_ci connections only.
    pub async fn user_by_login_text_query(
        &mut self,
        key: &str,
    ) -> Result<Option<UserRow>, UserLookupError> {
        self.user_text_query(Field::Login, key).await
    }

    /// Raw text SQL subset: WHERE user_email = ? LIMIT 1. Email is not unique
    /// in the reference schema; no particular duplicate winner or username/
    /// email preference is promised. This does not validate an email address.
    pub async fn user_by_email_text_query(
        &mut self,
        key: &str,
    ) -> Result<Option<UserRow>, UserLookupError> {
        self.user_text_query(Field::Email, key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_closed_identifiers_enter_sql_and_text_key_is_a_parameter() {
        let prefix = TablePrefix::new("wp_").unwrap();
        for (field, column) in [(Field::Login, "user_login"), (Field::Email, "user_email")] {
            let query = query(&prefix, field, "private-key' OR 1=1 --\0");
            assert!(
                query
                    .sql()
                    .as_str()
                    .ends_with(&format!(" WHERE {column} = ? LIMIT 1"))
            );
            assert!(!query.sql().as_str().contains("private-key"));
            assert!(!query.sql().as_str().contains("ORDER BY"));
            assert!(!query.sql().as_str().contains("WHERE CAST"));
        }
    }
}

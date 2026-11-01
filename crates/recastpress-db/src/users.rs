//! Editing-only raw account data. Never hand these rows to public serving.
//! Password/activation/capability bytes are preserved, not interpreted or used
//! to authorize requests. Debug deliberately contains only the numeric row ID.

use super::{EditingSnapshot, Table};
use sqlx::{MySql, QueryBuilder, Row};
use std::{fmt, num::NonZeroU16};

#[derive(Clone, Eq, PartialEq)]
pub struct UserRow {
    pub id: u64,
    pub user_login: Vec<u8>,
    pub user_pass: Vec<u8>,
    pub user_nicename: Vec<u8>,
    pub user_email: Vec<u8>,
    pub user_url: Vec<u8>,
    pub user_registered: String,
    pub user_activation_key: Vec<u8>,
    pub user_status: i32,
    pub display_name: Vec<u8>,
}

impl fmt::Debug for UserRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserRow")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct UserMetaRow {
    pub umeta_id: u64,
    pub user_id: u64,
    pub meta_key: Option<Vec<u8>>,
    pub meta_value: Option<Vec<u8>>,
}

impl fmt::Debug for UserMetaRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserMetaRow")
            .field("umeta_id", &self.umeta_id)
            .field("user_id", &self.user_id)
            .finish_non_exhaustive()
    }
}

impl EditingSnapshot {
    pub async fn users_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<UserRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT ID,CAST(user_login AS BINARY) AS user_login,CAST(user_pass AS BINARY) AS user_pass,CAST(user_nicename AS BINARY) AS user_nicename,CAST(user_email AS BINARY) AS user_email,CAST(user_url AS BINARY) AS user_url,CAST(user_registered AS CHAR) AS user_registered,CAST(user_activation_key AS BINARY) AS user_activation_key,user_status,CAST(display_name AS BINARY) AS display_name FROM ",
        );
        query.push(self.prefix.table(Table::Users));
        if let Some(id) = after {
            query.push(" WHERE ID > ").push_bind(id);
        }
        query.push(" ORDER BY ID ASC LIMIT ").push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
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
            })
            .collect()
    }

    pub async fn user_meta_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<UserMetaRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT umeta_id,user_id,CAST(meta_key AS BINARY) AS meta_key,CAST(meta_value AS BINARY) AS meta_value FROM ",
        );
        query.push(self.prefix.table(Table::UserMeta));
        if let Some(id) = after {
            query.push(" WHERE umeta_id > ").push_bind(id);
        }
        query
            .push(" ORDER BY umeta_id ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(UserMetaRow {
                    umeta_id: row.try_get("umeta_id")?,
                    user_id: row.try_get("user_id")?,
                    meta_key: row.try_get("meta_key")?,
                    meta_value: row.try_get("meta_value")?,
                })
            })
            .collect()
    }
}

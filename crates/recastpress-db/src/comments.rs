//! Editing-only comment export. Stored approval/type values are not permission
//! decisions, and reading migrated comments does not enable new submissions.

use super::{EditingSnapshot, Table};
use sqlx::{MySql, QueryBuilder, Row, mysql::MySqlRow};
use std::{fmt, num::NonZeroU16};

#[derive(Clone, Eq, PartialEq)]
pub struct CommentRow {
    pub comment_id: u64,
    pub comment_post_id: u64,
    pub comment_author: Vec<u8>,
    pub comment_author_email: Vec<u8>,
    pub comment_author_url: Vec<u8>,
    pub comment_author_ip: Vec<u8>,
    pub comment_date: String,
    pub comment_date_gmt: String,
    pub comment_content: Vec<u8>,
    pub comment_karma: i32,
    pub comment_approved: Vec<u8>,
    pub comment_agent: Vec<u8>,
    pub comment_type: Vec<u8>,
    pub comment_parent: u64,
    pub user_id: u64,
}

impl fmt::Debug for CommentRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommentRow")
            .field("comment_id", &self.comment_id)
            .field("comment_post_id", &self.comment_post_id)
            .finish_non_exhaustive()
    }
}

impl CommentRow {
    fn decode(row: MySqlRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            comment_id: row.try_get("comment_ID")?,
            comment_post_id: row.try_get("comment_post_ID")?,
            comment_author: row.try_get("comment_author")?,
            comment_author_email: row.try_get("comment_author_email")?,
            comment_author_url: row.try_get("comment_author_url")?,
            comment_author_ip: row.try_get("comment_author_IP")?,
            comment_date: row.try_get("comment_date")?,
            comment_date_gmt: row.try_get("comment_date_gmt")?,
            comment_content: row.try_get("comment_content")?,
            comment_karma: row.try_get("comment_karma")?,
            comment_approved: row.try_get("comment_approved")?,
            comment_agent: row.try_get("comment_agent")?,
            comment_type: row.try_get("comment_type")?,
            comment_parent: row.try_get("comment_parent")?,
            user_id: row.try_get("user_id")?,
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CommentMetaRow {
    pub meta_id: u64,
    pub comment_id: u64,
    pub meta_key: Option<Vec<u8>>,
    pub meta_value: Option<Vec<u8>>,
}

impl fmt::Debug for CommentMetaRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommentMetaRow")
            .field("meta_id", &self.meta_id)
            .field("comment_id", &self.comment_id)
            .finish_non_exhaustive()
    }
}

const COMMENT_FIELDS: &str = "comment_ID,comment_post_ID,CAST(comment_author AS BINARY) AS comment_author,CAST(comment_author_email AS BINARY) AS comment_author_email,CAST(comment_author_url AS BINARY) AS comment_author_url,CAST(comment_author_IP AS BINARY) AS comment_author_IP,CAST(comment_date AS CHAR) AS comment_date,CAST(comment_date_gmt AS CHAR) AS comment_date_gmt,CAST(comment_content AS BINARY) AS comment_content,comment_karma,CAST(comment_approved AS BINARY) AS comment_approved,CAST(comment_agent AS BINARY) AS comment_agent,CAST(comment_type AS BINARY) AS comment_type,comment_parent,user_id";

impl EditingSnapshot {
    pub async fn comment(&mut self, id: u64) -> Result<Option<CommentRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(COMMENT_FIELDS)
            .push(" FROM ")
            .push(self.prefix.table(Table::Comments))
            .push(" WHERE comment_ID = ")
            .push_bind(id);
        query
            .build()
            .fetch_optional(&mut *self.transaction)
            .await?
            .map(CommentRow::decode)
            .transpose()
    }

    /// None includes stored zero IDs. Status/type filtering belongs to a
    /// separately validated projection, never to this raw migration reader.
    pub async fn comments_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<CommentRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(COMMENT_FIELDS)
            .push(" FROM ")
            .push(self.prefix.table(Table::Comments));
        if let Some(id) = after {
            query.push(" WHERE comment_ID > ").push_bind(id);
        }
        query
            .push(" ORDER BY comment_ID ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(CommentRow::decode)
            .collect()
    }

    /// Preserve NULL/empty/duplicates and orphan references without a join.
    pub async fn comment_meta_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<CommentMetaRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT meta_id,comment_id,CAST(meta_key AS BINARY) AS meta_key,CAST(meta_value AS BINARY) AS meta_value FROM ",
        );
        query.push(self.prefix.table(Table::CommentMeta));
        if let Some(id) = after {
            query.push(" WHERE meta_id > ").push_bind(id);
        }
        query
            .push(" ORDER BY meta_id ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(CommentMetaRow {
                    meta_id: row.try_get("meta_id")?,
                    comment_id: row.try_get("comment_id")?,
                    meta_key: row.try_get("meta_key")?,
                    meta_value: row.try_get("meta_value")?,
                })
            })
            .collect()
    }
}

//! Preserve legacy link-manager data without treating visibility as public
//! authorization, sanitizing stored text, or enabling a new link editor.

use super::{EditingSnapshot, Table};
use sqlx::{MySql, QueryBuilder, Row};
use std::{fmt, num::NonZeroU16};

#[derive(Clone, Eq, PartialEq)]
pub struct LinkRow {
    pub link_id: u64,
    pub link_url: Vec<u8>,
    pub link_name: Vec<u8>,
    pub link_image: Vec<u8>,
    pub link_target: Vec<u8>,
    pub link_description: Vec<u8>,
    pub link_visible: Vec<u8>,
    pub link_owner: u64,
    pub link_rating: i32,
    pub link_updated: String,
    pub link_rel: Vec<u8>,
    pub link_notes: Vec<u8>,
    pub link_rss: Vec<u8>,
}

impl fmt::Debug for LinkRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkRow")
            .field("link_id", &self.link_id)
            .finish_non_exhaustive()
    }
}

impl EditingSnapshot {
    pub async fn links_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<LinkRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT link_id,CAST(link_url AS BINARY) AS link_url,CAST(link_name AS BINARY) AS link_name,CAST(link_image AS BINARY) AS link_image,CAST(link_target AS BINARY) AS link_target,CAST(link_description AS BINARY) AS link_description,CAST(link_visible AS BINARY) AS link_visible,link_owner,link_rating,CAST(link_updated AS CHAR) AS link_updated,CAST(link_rel AS BINARY) AS link_rel,CAST(link_notes AS BINARY) AS link_notes,CAST(link_rss AS BINARY) AS link_rss FROM ",
        );
        query.push(self.prefix.table(Table::Links));
        if let Some(id) = after {
            query.push(" WHERE link_id > ").push_bind(id);
        }
        query
            .push(" ORDER BY link_id ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(LinkRow {
                    link_id: row.try_get("link_id")?,
                    link_url: row.try_get("link_url")?,
                    link_name: row.try_get("link_name")?,
                    link_image: row.try_get("link_image")?,
                    link_target: row.try_get("link_target")?,
                    link_description: row.try_get("link_description")?,
                    link_visible: row.try_get("link_visible")?,
                    link_owner: row.try_get("link_owner")?,
                    link_rating: row.try_get("link_rating")?,
                    link_updated: row.try_get("link_updated")?,
                    link_rel: row.try_get("link_rel")?,
                    link_notes: row.try_get("link_notes")?,
                    link_rss: row.try_get("link_rss")?,
                })
            })
            .collect()
    }
}

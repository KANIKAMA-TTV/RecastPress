//! Raw taxonomy export, without joins, visibility decisions or PHP decoding.
//! Orphan references and stored counts are retained rather than repaired.

use super::{EditingSnapshot, Table};
use sqlx::{MySql, QueryBuilder, Row};
use std::{fmt, num::NonZeroU16};

#[derive(Clone, Eq, PartialEq)]
pub struct TermRow {
    pub term_id: u64,
    pub name: Vec<u8>,
    pub slug: Vec<u8>,
    pub term_group: i64,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TermTaxonomyRow {
    pub term_taxonomy_id: u64,
    pub term_id: u64,
    pub taxonomy: Vec<u8>,
    pub description: Vec<u8>,
    pub parent: u64,
    pub count: i64,
}

/// The entire primary key is needed to resume within a single object's terms.
/// None starts before (0, 0); Some((0, 0)) would skip that stored relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TermRelationshipKey {
    pub object_id: u64,
    pub term_taxonomy_id: u64,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TermRelationshipRow {
    pub key: TermRelationshipKey,
    pub term_order: i32,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TermMetaRow {
    pub meta_id: u64,
    pub term_id: u64,
    pub meta_key: Option<Vec<u8>>,
    pub meta_value: Option<Vec<u8>>,
}

impl fmt::Debug for TermRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TermRow")
            .field("term_id", &self.term_id)
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for TermTaxonomyRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TermTaxonomyRow")
            .field("term_taxonomy_id", &self.term_taxonomy_id)
            .field("term_id", &self.term_id)
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for TermRelationshipRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TermRelationshipRow")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for TermMetaRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TermMetaRow")
            .field("meta_id", &self.meta_id)
            .field("term_id", &self.term_id)
            .finish_non_exhaustive()
    }
}

impl EditingSnapshot {
    pub async fn terms_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<TermRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT term_id,CAST(name AS BINARY) AS name,CAST(slug AS BINARY) AS slug,term_group FROM ",
        );
        query.push(self.prefix.table(Table::Terms));
        if let Some(id) = after {
            query.push(" WHERE term_id > ").push_bind(id);
        }
        query
            .push(" ORDER BY term_id ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(TermRow {
                    term_id: row.try_get("term_id")?,
                    name: row.try_get("name")?,
                    slug: row.try_get("slug")?,
                    term_group: row.try_get("term_group")?,
                })
            })
            .collect()
    }

    pub async fn term_taxonomy_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<TermTaxonomyRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT term_taxonomy_id,term_id,CAST(taxonomy AS BINARY) AS taxonomy,CAST(description AS BINARY) AS description,parent,count FROM ",
        );
        query.push(self.prefix.table(Table::TermTaxonomy));
        if let Some(id) = after {
            query.push(" WHERE term_taxonomy_id > ").push_bind(id);
        }
        query
            .push(" ORDER BY term_taxonomy_id ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(TermTaxonomyRow {
                    term_taxonomy_id: row.try_get("term_taxonomy_id")?,
                    term_id: row.try_get("term_id")?,
                    taxonomy: row.try_get("taxonomy")?,
                    description: row.try_get("description")?,
                    parent: row.try_get("parent")?,
                    count: row.try_get("count")?,
                })
            })
            .collect()
    }

    /// Lexicographic primary-key pagination. Never use object_id alone as the
    /// continuation: one object can have more relationships than a page holds.
    pub async fn term_relationships_page(
        &mut self,
        after: Option<TermRelationshipKey>,
        limit: NonZeroU16,
    ) -> Result<Vec<TermRelationshipRow>, sqlx::Error> {
        let mut query =
            QueryBuilder::<MySql>::new("SELECT object_id,term_taxonomy_id,term_order FROM ");
        query.push(self.prefix.table(Table::TermRelationships));
        if let Some(key) = after {
            query
                .push(" WHERE (object_id > ")
                .push_bind(key.object_id)
                .push(" OR (object_id = ")
                .push_bind(key.object_id)
                .push(" AND term_taxonomy_id > ")
                .push_bind(key.term_taxonomy_id)
                .push("))");
        }
        query
            .push(" ORDER BY object_id ASC,term_taxonomy_id ASC LIMIT ")
            .push_bind(limit.get());
        query
            .build()
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(TermRelationshipRow {
                    key: TermRelationshipKey {
                        object_id: row.try_get("object_id")?,
                        term_taxonomy_id: row.try_get("term_taxonomy_id")?,
                    },
                    term_order: row.try_get("term_order")?,
                })
            })
            .collect()
    }

    pub async fn term_meta_page(
        &mut self,
        after: Option<u64>,
        limit: NonZeroU16,
    ) -> Result<Vec<TermMetaRow>, sqlx::Error> {
        let mut query = QueryBuilder::<MySql>::new(
            "SELECT meta_id,term_id,CAST(meta_key AS BINARY) AS meta_key,CAST(meta_value AS BINARY) AS meta_value FROM ",
        );
        query.push(self.prefix.table(Table::TermMeta));
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
                Ok(TermMetaRow {
                    meta_id: row.try_get("meta_id")?,
                    term_id: row.try_get("term_id")?,
                    meta_key: row.try_get("meta_key")?,
                    meta_value: row.try_get("meta_value")?,
                })
            })
            .collect()
    }
}

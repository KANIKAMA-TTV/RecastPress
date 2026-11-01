//! RecastPress primitives. This crate has no database, PHP, or network access.
//!
//! Public rendering receives read-only data through a separate capability boundary.
//! These APIs are experimental and do not establish WordPress compatibility.

pub mod cache;
pub mod publication;
pub mod resource_budget;
pub mod settings;

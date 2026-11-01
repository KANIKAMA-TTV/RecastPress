//! Read-only restore after the fixture producer has exited. This is neither a
//! process-crash test nor production source identity / snapshot coordination.

use mysql_async::{Conn, prelude::Queryable};
use recastpress_db::{
    PostMetaRow,
    meta_view::{PostMetaView, SearchPlan},
};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Stored = (Vec<u8>, Vec<u8>, u64, u64, Vec<u8>, Vec<u8>);
type EncodedRows = Vec<(u64, u64, Option<Vec<u8>>, Option<Vec<u8>>)>;

pub(crate) struct Observations {
    pub(crate) source_file: String,
    pub(crate) position: u64,
    pub(crate) rows: usize,
}

pub(crate) fn valid_source_file(file: &str) -> bool {
    file.strip_prefix("probe-bin.")
        .is_some_and(|suffix| suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

async fn stored(connection: &mut Conn) -> Result<Stored> {
    connection.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_projection WHERE id=1")
        .await?.ok_or_else(|| "saved postmeta projection missing".into())
}

fn decode_saved(
    stored: &Stored,
    epoch: &[u8],
    file: &str,
    position: u64,
    row_count: usize,
) -> Result<Vec<PostMetaRow>> {
    if !valid_source_file(file)
        || position <= 4
        || row_count > 64
        || stored.0 != epoch
        || stored.1 != file.as_bytes()
        || stored.2 < 4
        || stored.2 >= stored.3
        || stored.3 != position
    {
        return Err("saved postmeta context, row allowance or coordinates differ".into());
    }
    if stored.4.len() > 1024 * 1024 || stored.5.len() > 1024 * 1024 {
        return Err("saved postmeta fixture payload bound exceeded".into());
    }
    // The receipt contains opaque row-event payloads, not complete wire frames:
    // validating this shape is NOT a new checksum or source-authenticity proof.
    let receipt: Vec<Vec<u8>> = serde_json::from_slice(&stored.4)?;
    if receipt.is_empty() || receipt.len() > 8 || receipt.iter().any(Vec::is_empty) {
        return Err("saved postmeta receipt shape differs".into());
    }
    let rows: EncodedRows = serde_json::from_slice(&stored.5)?;
    if rows.len() != row_count {
        return Err("saved postmeta row count differs".into());
    }
    Ok(rows
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect())
}

pub(crate) async fn verify(
    connection: &mut Conn,
    epoch: &[u8],
    file: &str,
    position: u64,
    row_count: usize,
) -> Result<Observations> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
        return Err("fresh restore requires the owned disposable fixture".into());
    }
    let before = stored(connection).await?;
    let decoded = decode_saved(&before, epoch, file, position, row_count)?;
    let count: u64 = connection
        .query_first("SELECT COUNT(*) FROM rp_postmeta_projection")
        .await?
        .ok_or("saved projection table count missing")?;
    if count != 1 {
        return Err("saved projection is not the fixture singleton".into());
    }
    let files: Vec<mysql_async::Row> = connection.query("SHOW BINARY LOGS").await?;
    let mut found = false;
    for mut entry in files {
        let name: Vec<u8> = entry.take("Log_name").ok_or("log index name missing")?;
        let size: u64 = entry.take("File_size").ok_or("log index size missing")?;
        if name == file.as_bytes() {
            if found || size < position {
                return Err("saved postmeta position is outside the fixture log".into());
            }
            found = true;
        }
    }
    if !found {
        return Err("saved postmeta source file is absent".into());
    }
    // The owned fixture is quiescent here. Equality to current SQL is not a
    // general guarantee that historical checkpoints equal a moving source.
    let live: EncodedRows = connection.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM wp_postmeta ORDER BY meta_id LIMIT 65").await?;
    let live: Vec<_> = live
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect();
    if decoded != live || live.len() != row_count {
        return Err("fresh restored rows differ from quiescent fixture SQL".into());
    }
    let mut view =
        PostMetaView::new(decoded).map_err(|_| "duplicate fresh restored metadata ID")?;
    if view.build_key_index(64) != SearchPlan::ExactKeyIndex {
        return Err("fresh restored index was not rebuilt".into());
    }
    let mut keys: BTreeSet<_> = live.iter().map(|row| row.meta_key.clone()).collect();
    keys.insert(None);
    keys.insert(Some(Vec::new()));
    keys.insert(Some(b"fresh-restore-missing-key".to_vec()));
    for key in keys {
        if !view
            .matching_key(key.as_deref())
            .eq(live.iter().filter(|row| row.meta_key == key))
        {
            return Err("fresh restored index differs from fixture SQL rows".into());
        }
    }
    if stored(connection).await? != before {
        return Err("read-only fresh restore changed the saved projection".into());
    }
    Ok(Observations {
        source_file: file.to_owned(),
        position,
        rows: row_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Stored {
        (
            b"fixture".to_vec(),
            b"probe-bin.000001".to_vec(),
            10,
            20,
            br#"[[1,2,3]]"#.to_vec(),
            br#"[[0,18446744073709551615,null,[]]]"#.to_vec(),
        )
    }

    #[test]
    fn fresh_snapshot_validation_keeps_uint64_null_and_empty_bytes() {
        let rows = decode_saved(&fixture(), b"fixture", "probe-bin.000001", 20, 1).unwrap();
        assert_eq!(rows[0].meta_id, 0);
        assert_eq!(rows[0].post_id, u64::MAX);
        assert_eq!(rows[0].meta_key, None);
        assert_eq!(rows[0].meta_value, Some(Vec::new()));
    }

    #[test]
    fn fresh_snapshot_validation_rejects_context_coordinates_receipt_and_row_count() {
        let original = fixture();
        for (epoch, file, position, count) in [
            (b"other".as_slice(), "probe-bin.000001", 20, 1),
            (b"fixture", "probe-bin.000002", 20, 1),
            (b"fixture", "../probe-bin.000001", 20, 1),
            (b"fixture", "probe-bin.000001", 21, 1),
            (b"fixture", "probe-bin.000001", 20, 0),
            (b"fixture", "probe-bin.000001", 20, 65),
        ] {
            assert!(decode_saved(&original, epoch, file, position, count).is_err());
        }
        for receipt in [b"[]".as_slice(), b"[[]]", b"[null]", b"not json"] {
            let mut invalid = original.clone();
            invalid.4 = receipt.to_vec();
            assert!(decode_saved(&invalid, b"fixture", "probe-bin.000001", 20, 1).is_err());
        }
        let mut invalid = original;
        invalid.2 = invalid.3;
        assert!(decode_saved(&invalid, b"fixture", "probe-bin.000001", 20, 1).is_err());
    }
}

//! Toy InnoDB projection+receipt+position store inside the owned fixture only.
//! JSON encodings, source epoch and SQL storage are NOT production contracts.

use crate::{buffer_adapter::Position, require_fixture_tls};
use mysql_async::{Conn, OptsBuilder, TxOpts, prelude::Queryable};
use recastpress_db::{PostMetaRow, change_buffer::CommittedBatch};
use std::fmt;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type EncodedRows = Vec<(u64, u64, Option<Vec<u8>>, Option<Vec<u8>>)>;
type Stored = (Vec<u8>, Vec<u8>, u64, u64, Vec<u8>, Vec<u8>);

#[derive(Debug, PartialEq)]
enum Fault {
    SourceContext,
    ReplayConflict,
    PositionGap,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "projection journal fault: {self:?}")
    }
}
impl std::error::Error for Fault {}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Receipt {
    Applied,
    Replay,
}

pub(crate) struct Journal {
    connection: Conn,
    epoch: Vec<u8>,
    file: Vec<u8>,
}

pub(crate) struct Restored {
    pub(crate) position: u64,
    pub(crate) rows: Vec<PostMetaRow>,
}

impl Journal {
    pub(crate) async fn initialize(
        options: OptsBuilder,
        epoch: &[u8],
        file: &[u8],
        start: u64,
        rows: &[PostMetaRow],
    ) -> Result<Self> {
        if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
            return Err("projection journal requires the owned fixture".into());
        }
        let mut connection = Conn::new(options).await?;
        require_fixture_tls(&mut connection).await?;
        connection.query_drop("CREATE TABLE rp_postmeta_projection (id TINYINT UNSIGNED PRIMARY KEY,epoch VARBINARY(64) NOT NULL,log_file VARBINARY(255) NOT NULL,from_position BIGINT UNSIGNED NOT NULL,through_position BIGINT UNSIGNED NOT NULL,receipt LONGBLOB NOT NULL,row_data LONGBLOB NOT NULL) ENGINE=InnoDB").await?;
        connection
            .exec_drop(
                "INSERT INTO rp_postmeta_projection VALUES (1,?,?,?,?,?,?)",
                (epoch, file, start, start, b"".as_slice(), encode(rows)?),
            )
            .await?;
        Ok(Self {
            connection,
            epoch: epoch.to_vec(),
            file: file.to_vec(),
        })
    }

    pub(crate) async fn apply(
        &mut self,
        batch: &CommittedBatch<Position>,
        rows: &[PostMetaRow],
    ) -> Result<Receipt> {
        if !batch.from().matches_context(&self.epoch, &self.file)
            || !batch.through().matches_context(&self.epoch, &self.file)
            || !batch.from().same_session(batch.through())
        {
            return Err(Fault::SourceContext.into());
        }
        let data = encode(rows)?;
        let receipt = serde_json::to_vec(&batch.rows().map(<[u8]>::to_vec).collect::<Vec<_>>())?;
        if data.len() > 1024 * 1024 || receipt.len() > 1024 * 1024 {
            return Err("projection journal fixture payload bound exceeded".into());
        }
        let mut tx = self.connection.start_transaction(TxOpts::default()).await?;
        let stored: Stored = tx.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_projection WHERE id=1 FOR UPDATE").await?.ok_or("projection checkpoint missing")?;
        if stored.0 != self.epoch || stored.1 != self.file {
            tx.rollback().await?;
            return Err(Fault::SourceContext.into());
        }
        if stored.3 == batch.through().offset() {
            let same = stored.2 == batch.from().offset() && stored.4 == receipt && stored.5 == data;
            tx.rollback().await?;
            return if same {
                Ok(Receipt::Replay)
            } else {
                Err(Fault::ReplayConflict.into())
            };
        }
        if stored.3 != batch.from().offset() {
            tx.rollback().await?;
            return Err(Fault::PositionGap.into());
        }
        tx.exec_drop("UPDATE rp_postmeta_projection SET from_position=?,through_position=?,receipt=?,row_data=? WHERE id=1", (batch.from().offset(), batch.through().offset(), receipt, data)).await?;
        if tx.affected_rows() != 1 {
            return Err("projection checkpoint update count differs".into());
        }
        tx.commit().await?;
        Ok(Receipt::Applied)
    }

    /// Exercise actual InnoDB rollback and a separate reader while an update is
    /// uncommitted. Only the new fixture projection row is locked or modified.
    pub(crate) async fn verify_rollback(
        &mut self,
        batch: &CommittedBatch<Position>,
        rows: &[PostMetaRow],
        options: OptsBuilder,
    ) -> Result<()> {
        let prior: Stored = self.connection.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_projection WHERE id=1").await?.ok_or("no pre-rollback projection")?;
        if prior.0 != self.epoch || prior.1 != self.file || prior.3 != batch.from().offset() {
            return Err("rollback probe did not start at the committed boundary".into());
        }
        let data = encode(rows)?;
        let mut observer = Conn::new(options).await?;
        require_fixture_tls(&mut observer).await?;
        let mut tx = self.connection.start_transaction(TxOpts::default()).await?;
        tx.exec_drop("UPDATE rp_postmeta_projection SET from_position=?,through_position=?,receipt=?,row_data=? WHERE id=1", (batch.from().offset(),batch.through().offset(),b"rolled-back receipt".as_slice(),&data)).await?;
        let changed: (u64, Vec<u8>) = tx
            .query_first("SELECT through_position,row_data FROM rp_postmeta_projection WHERE id=1")
            .await?
            .ok_or("no uncommitted projection")?;
        if changed.0 != batch.through().offset() || changed.1 != data {
            return Err("rollback probe did not stage its data/position pair".into());
        }
        let visible: Stored = observer.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_projection WHERE id=1").await?.ok_or("no observer projection")?;
        crate::close_fixture_connection(observer, "postmeta_rollback_observer").await?;
        if visible != prior {
            return Err("another SQL connection observed an uncommitted projection".into());
        }
        tx.rollback().await?;
        let after: Stored = self.connection.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_projection WHERE id=1").await?.ok_or("no post-rollback projection")?;
        if after != prior {
            return Err("rollback changed the committed projection/position/receipt".into());
        }
        Ok(())
    }

    pub(crate) async fn verify_conflicting_replay(
        &mut self,
        batch: &CommittedBatch<Position>,
        rows: &[PostMetaRow],
    ) -> Result<()> {
        let mut altered = rows.to_vec();
        let first = altered
            .first_mut()
            .ok_or("no projection row for conflict check")?;
        first.meta_value = Some(b"conflicting replay must not persist".to_vec());
        require_fault(self.apply(batch, &altered).await, Fault::ReplayConflict)?;
        let saved = self.read().await?;
        if saved.position != batch.through().offset() || saved.rows != rows {
            return Err("conflicting replay changed stored projection".into());
        }
        Ok(())
    }

    async fn read(&mut self) -> Result<Restored> {
        let stored: Stored = self.connection.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_projection WHERE id=1").await?.ok_or("projection checkpoint missing")?;
        if stored.0 != self.epoch || stored.1 != self.file {
            return Err(Fault::SourceContext.into());
        }
        if stored.5.len() > 1024 * 1024 {
            return Err("restored projection size differs".into());
        }
        let encoded: EncodedRows = serde_json::from_slice(&stored.5)?;
        if encoded.len() > 64 {
            return Err("restored projection row bound exceeded".into());
        }
        Ok(Restored {
            position: stored.3,
            rows: encoded
                .into_iter()
                .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
                    meta_id,
                    post_id,
                    meta_key,
                    meta_value,
                })
                .collect(),
        })
    }

    pub(crate) async fn reopen(&mut self, options: OptsBuilder) -> Result<Restored> {
        let before: u64 = self
            .connection
            .query_first("SELECT CONNECTION_ID()")
            .await?
            .ok_or("no old projection connection ID")?;
        let mut new = Conn::new(options).await?;
        require_fixture_tls(&mut new).await?;
        let after: u64 = new
            .query_first("SELECT CONNECTION_ID()")
            .await?
            .ok_or("no new projection connection ID")?;
        if before == after {
            return Err("projection connection was not replaced".into());
        }
        let old = std::mem::replace(&mut self.connection, new);
        crate::close_fixture_connection(old, "postmeta_journal_old").await?;
        let restored = self.read().await?;
        let expected_epoch = self.epoch.clone();
        self.epoch.push(0);
        let rejected_epoch = self.read().await;
        self.epoch = expected_epoch;
        require_fault(rejected_epoch, Fault::SourceContext)?;
        let expected_file = self.file.clone();
        self.file.push(0);
        let rejected_file = self.read().await;
        self.file = expected_file;
        require_fault(rejected_file, Fault::SourceContext)?;
        Ok(restored)
    }

    pub(crate) async fn verify(&mut self, position: u64, rows: &[PostMetaRow]) -> Result<()> {
        let saved = self.read().await?;
        if saved.position != position || saved.rows != rows {
            return Err("durable projection/position differ from committed view".into());
        }
        Ok(())
    }
}

fn encode(rows: &[PostMetaRow]) -> Result<Vec<u8>> {
    if rows.len() > 64 {
        return Err("projection journal fixture row bound exceeded".into());
    }
    Ok(serde_json::to_vec(
        &rows
            .iter()
            .map(|row| (row.meta_id, row.post_id, &row.meta_key, &row.meta_value))
            .collect::<Vec<_>>(),
    )?)
}

fn require_fault<T>(result: Result<T>, expected: Fault) -> Result<()> {
    match result {
        Err(error) if error.downcast_ref::<Fault>() == Some(&expected) => Ok(()),
        _ => Err("projection negative fixture did not report its required typed fault".into()),
    }
}

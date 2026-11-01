//! Fixture-only atomic journal, not the production internal data/schema design.

use crate::buffer_adapter::Position;
use mysql_async::{Conn, OptsBuilder, TxOpts, prelude::Queryable};
use recastpress_db::change_buffer::CommittedBatch;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
enum JournalFault {
    PayloadConflict,
    SourceContext,
}
impl std::fmt::Display for JournalFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::PayloadConflict => "conflicting payload at the same durable position",
            Self::SourceContext => "durable checkpoint belongs to another fixture source context",
        })
    }
}
impl std::error::Error for JournalFault {}

pub(crate) async fn create_tables(connection: &mut Conn) -> Result<()> {
    connection.query_drop("CREATE TABLE rp_cdc_checkpoint (id TINYINT UNSIGNED PRIMARY KEY, source_epoch VARBINARY(64) NOT NULL, log_file VARBINARY(255) NOT NULL, log_pos BIGINT UNSIGNED NOT NULL) ENGINE=InnoDB").await?;
    connection.query_drop("CREATE TABLE rp_cdc_journal (through_pos BIGINT UNSIGNED NOT NULL, event_ordinal BIGINT UNSIGNED NOT NULL, payload LONGBLOB NOT NULL, PRIMARY KEY(through_pos,event_ordinal)) ENGINE=InnoDB").await?;
    Ok(())
}

pub(crate) struct AtomicJournal<'a> {
    connection: &'a mut Conn,
    epoch: Vec<u8>,
    file: Vec<u8>,
    pub(crate) fresh: usize,
    pub(crate) replays: usize,
    pub(crate) conflict_rejected: bool,
    pub(crate) reopened: bool,
    pub(crate) wrong_context_rejected: bool,
}

impl<'a> AtomicJournal<'a> {
    pub(crate) async fn initialize(
        connection: &'a mut Conn,
        epoch: &[u8],
        file: &[u8],
        initial: u64,
    ) -> Result<Self> {
        connection
            .exec_drop(
                "INSERT INTO rp_cdc_checkpoint VALUES (1,?,?,?)",
                (epoch, file, initial),
            )
            .await?;
        Ok(Self {
            connection,
            epoch: epoch.to_vec(),
            file: file.to_vec(),
            fresh: 0,
            replays: 0,
            conflict_rejected: false,
            reopened: false,
            wrong_context_rejected: false,
        })
    }

    /// Reopen the fixture journal connection, then load its persisted position.
    /// The epoch remains the owned fixture UUID; production source identity and
    /// reconnecting across server replacement/restoration remain unimplemented.
    pub(crate) async fn reopen_checkpoint(&mut self, options: OptsBuilder) -> Result<u64> {
        let (old_id,): (u64,) = self
            .connection
            .query_first("SELECT CONNECTION_ID()")
            .await?
            .ok_or("no original connection ID")?;
        let mut replacement = Conn::new(options).await?;
        crate::require_fixture_tls(&mut replacement).await?;
        let (new_id,): (u64,) = replacement
            .query_first("SELECT CONNECTION_ID()")
            .await?
            .ok_or("no replacement connection ID")?;
        if new_id == old_id {
            return Err("journal did not establish a new connection".into());
        }
        let old = std::mem::replace(self.connection, replacement);
        crate::close_fixture_connection(old, "original_journal_old").await?;
        for (epoch, file) in [
            (b"not-the-owned-fixture".as_slice(), self.file.as_slice()),
            (self.epoch.as_slice(), b"not-the-owned-binlog".as_slice()),
        ] {
            match Self::read_checkpoint(self.connection, epoch, file).await {
                Err(error)
                    if matches!(
                        error.downcast_ref::<JournalFault>(),
                        Some(JournalFault::SourceContext)
                    ) => {}
                Err(error) => return Err(error),
                Ok(_) => return Err("checkpoint accepted an unrelated source context".into()),
            }
        }
        self.wrong_context_rejected = true;
        let position = Self::read_checkpoint(self.connection, &self.epoch, &self.file).await?;
        self.reopened = true;
        Ok(position)
    }

    async fn read_checkpoint(
        connection: &mut Conn,
        expected_epoch: &[u8],
        expected_file: &[u8],
    ) -> Result<u64> {
        let (epoch, file, position): (Vec<u8>, Vec<u8>, u64) = connection
            .query_first("SELECT source_epoch,log_file,log_pos FROM rp_cdc_checkpoint WHERE id=1")
            .await?
            .ok_or("no durable checkpoint to resume")?;
        if epoch != expected_epoch || file != expected_file {
            return Err(JournalFault::SourceContext.into());
        }
        Ok(position)
    }

    pub(crate) async fn apply(&mut self, batch: &CommittedBatch<Position>) -> Result<()> {
        if !batch.from().matches_context(&self.epoch, &self.file)
            || !batch.through().matches_context(&self.epoch, &self.file)
            || !batch.from().same_session(batch.through())
        {
            return Err("batch source context differs from journal".into());
        }
        let from = batch.from().offset();
        let through = batch.through().offset();
        let payloads: Vec<_> = batch.rows().map(<[u8]>::to_vec).collect();
        self.apply_payloads(from, through, payloads).await
    }

    async fn apply_payloads(
        &mut self,
        from: u64,
        through: u64,
        payloads: Vec<Vec<u8>>,
    ) -> Result<()> {
        let mut transaction = self.connection.start_transaction(TxOpts::default()).await?;
        let (epoch, file, current): (Vec<u8>, Vec<u8>, u64) = transaction
            .query_first(
                "SELECT source_epoch,log_file,log_pos FROM rp_cdc_checkpoint WHERE id=1 FOR UPDATE",
            )
            .await?
            .ok_or("journal checkpoint is missing")?;
        if epoch != self.epoch || file != self.file {
            return Err("durable source context changed".into());
        }
        if current == through {
            let persisted: Vec<(Vec<u8>,)> = transaction
                .exec(
                    "SELECT payload FROM rp_cdc_journal WHERE through_pos=? ORDER BY event_ordinal",
                    (through,),
                )
                .await?;
            if persisted
                .into_iter()
                .map(|(value,)| value)
                .collect::<Vec<_>>()
                != payloads
            {
                return Err(JournalFault::PayloadConflict.into());
            }
            transaction.commit().await?;
            self.replays += 1;
            return Ok(());
        }
        if current != from {
            return Err("durable checkpoint is not the batch predecessor".into());
        }
        for (index, payload) in payloads.iter().enumerate() {
            transaction
                .exec_drop(
                    "INSERT INTO rp_cdc_journal VALUES (?,?,?)",
                    (through, u64::try_from(index)?, payload.as_slice()),
                )
                .await?;
        }
        transaction
            .exec_drop(
                "UPDATE rp_cdc_checkpoint SET log_pos=? WHERE id=1",
                (through,),
            )
            .await?;
        transaction.commit().await?;
        self.fresh += 1;
        Ok(())
    }

    pub(crate) async fn verify_conflicting_replay(
        &mut self,
        batch: &CommittedBatch<Position>,
    ) -> Result<()> {
        let mut payloads: Vec<_> = batch.rows().map(<[u8]>::to_vec).collect();
        let first = payloads.first_mut().ok_or("no payload to conflict")?;
        if let Some(byte) = first.first_mut() {
            *byte ^= 0x80;
        } else {
            first.push(1);
        }
        match self
            .apply_payloads(batch.from().offset(), batch.through().offset(), payloads)
            .await
        {
            Err(error)
                if matches!(
                    error.downcast_ref::<JournalFault>(),
                    Some(JournalFault::PayloadConflict)
                ) =>
            {
                self.conflict_rejected = true;
                Ok(())
            }
            Err(error) => Err(error),
            Ok(()) => Err("conflicting replay was incorrectly accepted".into()),
        }
    }

    pub(crate) async fn verify(&mut self, expected: u64) -> Result<()> {
        let (epoch, file, position): (Vec<u8>, Vec<u8>, u64) = self
            .connection
            .query_first("SELECT source_epoch,log_file,log_pos FROM rp_cdc_checkpoint WHERE id=1")
            .await?
            .ok_or("no durable checkpoint")?;
        let (events, last): (u64, Option<u64>) = self
            .connection
            .query_first("SELECT COUNT(*),MAX(through_pos) FROM rp_cdc_journal")
            .await?
            .ok_or("no journal aggregate")?;
        if epoch != self.epoch
            || file != self.file
            || position != expected
            || events != 3
            || last != Some(expected)
            || self.fresh != 3
            || self.replays != 1
            || !self.conflict_rejected
            || !self.reopened
            || !self.wrong_context_rejected
        {
            return Err("durable journal/checkpoint or idempotent retry counts differ".into());
        }
        Ok(())
    }
}

use recastpress_db::change_buffer::{
    BufferError, ChangeBuffer, CommittedBatch, Event, Fault, Outcome,
};

fn staged() -> ChangeBuffer<u64> {
    let mut buffer = ChangeBuffer::new(10, 4096);
    assert_eq!(
        buffer.push(Event::Begin { start: 10, end: 11 }),
        Ok(Outcome::Staged)
    );
    assert_eq!(
        buffer.push(Event::Row {
            start: 11,
            end: 12,
            bytes: b"private row".to_vec()
        }),
        Ok(Outcome::Staged)
    );
    buffer
}

#[test]
fn only_a_complete_successfully_applied_transaction_advances_checkpoint() {
    let mut buffer = staged();
    assert_eq!(buffer.checkpoint(), &10);
    assert!(buffer.pending().is_none());
    buffer.push(Event::Advance { start: 12, end: 13 }).unwrap();
    buffer.push(Event::Commit { start: 13, end: 14 }).unwrap();
    let mut durable = (10, Vec::<Vec<u8>>::new());
    assert_eq!(buffer.apply_pending(|_| Err("disk full")), Err("disk full"));
    assert_eq!(buffer.checkpoint(), &10);
    assert_eq!(durable.0, 10);
    assert_eq!(
        buffer.push(Event::Begin { start: 14, end: 15 }),
        Err(BufferError::ApplyRequired)
    );
    assert!(
        buffer
            .apply_pending::<()>(|batch| {
                assert_eq!(batch.from(), &durable.0);
                durable = (*batch.through(), batch.rows().map(<[u8]>::to_vec).collect());
                Ok(())
            })
            .unwrap()
    );
    assert_eq!(buffer.checkpoint(), &14);
    assert_eq!(durable, (14, vec![b"private row".to_vec()]));
    assert!(
        !buffer
            .apply_pending::<()>(|_| panic!("must not apply twice"))
            .unwrap()
    );
}

#[test]
fn rollback_and_idle_progress_persist_only_the_position() {
    let mut buffer = staged();
    buffer.push(Event::Rollback { start: 12, end: 13 }).unwrap();
    assert_eq!(buffer.pending().unwrap().rows().len(), 0);
    buffer
        .apply_pending::<()>(|batch| {
            assert_eq!(batch.charged_bytes(), 0);
            Ok(())
        })
        .unwrap();
    assert_eq!(buffer.checkpoint(), &13);
    buffer.push(Event::Advance { start: 13, end: 20 }).unwrap();
    assert_eq!(buffer.checkpoint(), &13);
    buffer
        .apply_pending::<()>(|batch| {
            assert_eq!(batch.rows().len(), 0);
            Ok(())
        })
        .unwrap();
    assert_eq!(buffer.checkpoint(), &20);
}

#[test]
fn gaps_replayed_positions_and_unexpected_events_never_skip_or_reapply() {
    let mut buffer = staged();
    assert_eq!(
        buffer.push(Event::Commit { start: 13, end: 14 }),
        Err(BufferError::RecoveryRequired(Fault::PositionGap))
    );
    assert_eq!(buffer.checkpoint(), &10);
    assert!(buffer.pending().is_none());
    assert_eq!(
        buffer.push(Event::Commit { start: 12, end: 13 }),
        Err(BufferError::RecoveryRequired(Fault::PositionGap))
    );
    let mut buffer = ChangeBuffer::new(10, 4096);
    assert_eq!(
        buffer.push(Event::Row {
            start: 10,
            end: 11,
            bytes: vec![]
        }),
        Err(BufferError::RecoveryRequired(Fault::UnexpectedEvent))
    );
    let mut buffer = ChangeBuffer::new(10, 4096);
    assert_eq!(
        buffer.push(Event::Advance { start: 10, end: 10 }),
        Err(BufferError::RecoveryRequired(Fault::PositionGap))
    );
}

#[test]
fn loss_and_ddl_discard_incomplete_and_unapplied_transactions() {
    for committed in [false, true] {
        let mut buffer = staged();
        if committed {
            buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
        }
        buffer.disconnect();
        assert_eq!(buffer.fault(), Some(Fault::Disconnected));
        assert!(buffer.pending().is_none());
        assert_eq!(buffer.checkpoint(), &10);
        assert!(
            !buffer
                .apply_pending::<()>(|_| panic!("invalidated work"))
                .unwrap()
        );
        let mut buffer = staged();
        if committed {
            buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
        }
        assert_eq!(
            buffer.push(Event::Ddl),
            Err(BufferError::RecoveryRequired(Fault::SchemaChanged))
        );
        assert!(buffer.pending().is_none());
        assert_eq!(buffer.checkpoint(), &10);
    }
}

#[test]
fn retained_capacity_and_cumulative_rows_obey_payload_budget() {
    let mut buffer = ChangeBuffer::new(0, 100);
    buffer.push(Event::Begin { start: 0, end: 1 }).unwrap();
    let mut spare = Vec::with_capacity(1024);
    spare.push(1);
    assert_eq!(
        buffer.push(Event::Row {
            start: 1,
            end: 2,
            bytes: spare
        }),
        Err(BufferError::RecoveryRequired(Fault::PayloadBudgetExceeded))
    );
    assert_eq!(buffer.checkpoint(), &0);
    let mut buffer = ChangeBuffer::new(0, 100);
    buffer.push(Event::Begin { start: 0, end: 1 }).unwrap();
    buffer
        .push(Event::Row {
            start: 1,
            end: 2,
            bytes: vec![0; 20],
        })
        .unwrap();
    assert_eq!(
        buffer.push(Event::Row {
            start: 2,
            end: 3,
            bytes: vec![0; 20]
        }),
        Err(BufferError::RecoveryRequired(Fault::PayloadBudgetExceeded))
    );
}

#[test]
fn pressure_discards_whole_work_and_recovers_only_from_durable_position() {
    for committed in [false, true] {
        let mut buffer = staged();
        if committed {
            buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
        }
        let charged = buffer.charged_bytes();
        assert!(charged > 0);
        buffer.apply_payload_budget(charged).unwrap();
        assert_eq!(
            buffer.apply_payload_budget(0),
            Err(BufferError::RecoveryRequired(Fault::PayloadBudgetExceeded))
        );
        assert_eq!(buffer.charged_bytes(), 0);
        assert_eq!(buffer.checkpoint(), &10);
        assert!(buffer.pending().is_none());
        assert_eq!(
            buffer.apply_payload_budget(4096),
            Err(BufferError::RecoveryRequired(Fault::PayloadBudgetExceeded))
        );
    }
}

#[test]
fn source_identity_is_part_of_position_and_nested_transactions_are_rejected() {
    let mut buffer = ChangeBuffer::new(("source-a", 10), 4096);
    assert_eq!(
        buffer.push(Event::Begin {
            start: ("source-b", 10),
            end: ("source-b", 11)
        }),
        Err(BufferError::RecoveryRequired(Fault::PositionGap))
    );
    assert_eq!(buffer.checkpoint(), &("source-a", 10));
    let mut buffer = staged();
    assert_eq!(
        buffer.push(Event::Begin { start: 12, end: 13 }),
        Err(BufferError::RecoveryRequired(Fault::UnexpectedEvent))
    );
    assert_eq!(buffer.checkpoint(), &10);
    assert_eq!(buffer.charged_bytes(), 0);
}

struct AtomicStore {
    checkpoint: u64,
    rows: Vec<Vec<u8>>,
    writes: usize,
}

impl AtomicStore {
    fn apply(&mut self, batch: &CommittedBatch<u64>) -> Result<(), &'static str> {
        let rows: Vec<_> = batch.rows().map(<[u8]>::to_vec).collect();
        if self.checkpoint == *batch.through() {
            return if self.rows == rows {
                Ok(())
            } else {
                Err("conflicting batch at the same checkpoint")
            };
        }
        if self.checkpoint != *batch.from() {
            return Err("durable checkpoint differs");
        }
        (self.checkpoint, self.rows) = (*batch.through(), rows);
        self.writes += 1;
        Ok(())
    }
}

#[test]
fn lost_commit_ack_retries_without_double_application_and_rejects_conflicts() {
    let mut buffer = staged();
    buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
    let mut store = AtomicStore {
        checkpoint: 10,
        rows: vec![],
        writes: 0,
    };
    let result = buffer.apply_pending(|batch| {
        store.apply(batch)?;
        Err("commit acknowledgement lost")
    });
    assert_eq!(result, Err("commit acknowledgement lost"));
    assert_eq!(buffer.checkpoint(), &10);
    assert_eq!(store.checkpoint, 13);
    assert!(buffer.pending().is_some());
    assert!(buffer.apply_pending(|batch| store.apply(batch)).unwrap());
    assert_eq!(buffer.checkpoint(), &13);
    assert_eq!(store.writes, 1);
    let mut conflicting = ChangeBuffer::new(10, 4096);
    conflicting
        .push(Event::Begin { start: 10, end: 11 })
        .unwrap();
    conflicting
        .push(Event::Row {
            start: 11,
            end: 12,
            bytes: b"different content".to_vec(),
        })
        .unwrap();
    conflicting
        .push(Event::Commit { start: 12, end: 13 })
        .unwrap();
    assert_eq!(
        conflicting.apply_pending(|batch| store.apply(batch)),
        Err("conflicting batch at the same checkpoint")
    );
    assert_eq!(conflicting.checkpoint(), &10);
    assert_eq!(store.writes, 1);
}

#[test]
fn cancelled_async_apply_preserves_retry_after_an_ambiguous_commit() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let mut buffer = staged();
    buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
    let mut store = AtomicStore {
        checkpoint: 10,
        rows: vec![],
        writes: 0,
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut applying = Box::pin(buffer.apply_pending_async(async |batch| {
        store.apply(batch)?;
        std::future::pending::<()>().await;
        Ok::<(), &'static str>(())
    }));
    assert!(matches!(
        applying.as_mut().poll(&mut context),
        Poll::Pending
    ));
    drop(applying); // Simulate cancellation after durable commit, before its reply.
    assert_eq!(buffer.checkpoint(), &10);
    assert_eq!(store.checkpoint, 13);
    assert!(buffer.pending().is_some());
    let mut retrying = Box::pin(buffer.apply_pending_async(async |batch| store.apply(batch)));
    assert!(matches!(
        retrying.as_mut().poll(&mut context),
        Poll::Ready(Ok(true))
    ));
    drop(retrying);
    assert_eq!(buffer.checkpoint(), &13);
    assert_eq!(store.writes, 1);
    let mut empty = Box::pin(
        buffer.apply_pending_async(async |_| -> Result<(), ()> { panic!("no batch to apply") }),
    );
    assert!(matches!(
        empty.as_mut().poll(&mut context),
        Poll::Ready(Ok(false))
    ));
}

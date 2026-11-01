use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use recastpress_db::{
    PostMetaRow,
    budgeted_change_buffer::{BudgetedChangeBuffer, BudgetedPushError},
    budgeted_meta_view::BudgetedPostMetaView,
    change_buffer::{BufferError, CommittedBatch, Event, Fault, Outcome},
};

fn charge(bytes: &Vec<u8>) -> usize {
    bytes.capacity() + 2 * std::mem::size_of::<Vec<u8>>()
}

fn claims(pool: &BudgetPool, expected: usize) {
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::ChangeBuffer),
        expected
    );
}

fn staged(pool: BudgetPool) -> BudgetedChangeBuffer<u64> {
    let mut buffer = BudgetedChangeBuffer::new(10, 4096, pool);
    buffer.push(Event::Begin { start: 10, end: 11 }).unwrap();
    buffer
        .push(Event::Row {
            start: 11,
            end: 12,
            bytes: b"private\0\xff".to_vec(),
        })
        .unwrap();
    buffer
}

#[derive(Default)]
struct Store {
    checkpoint: u64,
    rows: Vec<Vec<u8>>,
    writes: usize,
}
impl Store {
    fn apply(&mut self, batch: &CommittedBatch<u64>) -> Result<(), &'static str> {
        let rows: Vec<_> = batch.rows().map(<[u8]>::to_vec).collect();
        if self.checkpoint == *batch.through() {
            return if self.rows == rows {
                Ok(())
            } else {
                Err("conflicting receipt")
            };
        }
        if self.checkpoint != *batch.from() {
            return Err("wrong checkpoint");
        }
        self.checkpoint = *batch.through();
        self.rows = rows;
        self.writes += 1;
        Ok(())
    }
}

#[test]
fn raw_capacity_and_row_allowance_remain_reserved_through_staging_and_pending() {
    let mut bytes = Vec::with_capacity(511);
    bytes.push(7);
    let first_charge = charge(&bytes);
    let second = Vec::with_capacity(19);
    let expected = first_charge + charge(&second);
    assert!(first_charge > bytes.len() + 2 * std::mem::size_of::<Vec<u8>>());
    let pool = BudgetPool::new(expected);
    let mut buffer = BudgetedChangeBuffer::new(0, expected, pool.clone());
    buffer.push(Event::Begin { start: 0, end: 1 }).unwrap();
    buffer
        .push(Event::Row {
            start: 1,
            end: 2,
            bytes,
        })
        .unwrap();
    claims(&pool, first_charge);
    assert_eq!(buffer.charged_bytes(), first_charge);
    assert_eq!(
        buffer.apply_pending(|_| -> Result<(), ()> { panic!("not pending") }),
        Ok(false)
    );
    claims(&pool, first_charge);
    assert_eq!(
        buffer.push(Event::Advance { start: 2, end: 3 }),
        Ok(Outcome::Staged)
    );
    assert_eq!(
        buffer.push(Event::Row {
            start: 3,
            end: 4,
            bytes: second
        }),
        Ok(Outcome::Staged)
    );
    assert_eq!(buffer.charged_bytes(), expected);
    claims(&pool, expected);
    assert_eq!(
        buffer.push(Event::Commit { start: 4, end: 5 }),
        Ok(Outcome::ApplyRequired)
    );
    claims(&pool, expected);
    let batch = buffer.pending().unwrap();
    assert_eq!(batch.charged_bytes(), expected);
    assert_eq!(
        batch.rows().collect::<Vec<_>>(),
        [b"\x07".as_slice(), b"".as_slice()]
    );
    assert_eq!(buffer.apply_pending(|_| Ok::<(), ()>(())), Ok(true));
    assert_eq!(*buffer.checkpoint(), 5);
    claims(&pool, 0);
}

#[test]
fn existing_validation_priority_runs_before_any_shared_pool_admission() {
    let pool = BudgetPool::new(0);
    let mut gap = BudgetedChangeBuffer::new(0, usize::MAX, pool.clone());
    assert_eq!(
        gap.push(Event::Row {
            start: 1,
            end: 2,
            bytes: vec![1]
        }),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::PositionGap
        )))
    );
    let mut idle = BudgetedChangeBuffer::new(0, usize::MAX, pool.clone());
    assert_eq!(
        idle.push(Event::Row {
            start: 0,
            end: 1,
            bytes: vec![1]
        }),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::UnexpectedEvent
        )))
    );
    let mut local = BudgetedChangeBuffer::new(0, 0, pool.clone());
    local.push(Event::Begin { start: 0, end: 1 }).unwrap();
    assert_eq!(
        local.push(Event::Row {
            start: 1,
            end: 2,
            bytes: vec![]
        }),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::PayloadBudgetExceeded
        )))
    );
    let mut pending = BudgetedChangeBuffer::new(0, usize::MAX, pool.clone());
    pending.push(Event::Advance { start: 0, end: 1 }).unwrap();
    assert_eq!(
        pending.push(Event::Row {
            start: 99,
            end: 100,
            bytes: vec![1]
        }),
        Err(BudgetedPushError::Buffer(BufferError::ApplyRequired))
    );
    assert_eq!(
        pending.push(Event::Ddl),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::SchemaChanged
        )))
    );
    assert_eq!(
        pending.push(Event::Begin { start: 1, end: 2 }),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::SchemaChanged
        )))
    );
    let mut global = BudgetedChangeBuffer::new(0, usize::MAX, pool.clone());
    global.push(Event::Begin { start: 0, end: 1 }).unwrap();
    assert_eq!(
        global.push(Event::Row {
            start: 1,
            end: 2,
            bytes: vec![]
        }),
        Err(BudgetedPushError::Budget(BudgetError::Exhausted))
    );
    claims(&pool, 0);
}

#[test]
fn failed_growth_discards_all_rows_and_latches_recovery_at_the_old_checkpoint() {
    let first = vec![1];
    let expected = charge(&first);
    let pool = BudgetPool::new(expected);
    let mut buffer = BudgetedChangeBuffer::new(0, usize::MAX, pool.clone());
    buffer.push(Event::Begin { start: 0, end: 1 }).unwrap();
    buffer
        .push(Event::Row {
            start: 1,
            end: 2,
            bytes: first,
        })
        .unwrap();
    claims(&pool, expected);
    assert_eq!(
        buffer.push(Event::Row {
            start: 2,
            end: 3,
            bytes: b"must not skip".to_vec()
        }),
        Err(BudgetedPushError::Budget(BudgetError::Exhausted))
    );
    assert_eq!(*buffer.checkpoint(), 0);
    assert!(buffer.pending().is_none());
    assert_eq!(buffer.charged_bytes(), 0);
    claims(&pool, 0);
    assert_eq!(
        buffer.push(Event::Commit { start: 3, end: 4 }),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::PayloadBudgetExceeded
        )))
    );
    assert_eq!(
        buffer.push(Event::Ddl),
        Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
            Fault::PayloadBudgetExceeded
        )))
    );
    let error = BudgetedPushError::Budget(BudgetError::Unavailable);
    fn is_error(_: &dyn std::error::Error) {}
    is_error(&error);
    assert!(!error.to_string().contains("must not skip"));
}

#[test]
fn acknowledged_loss_retains_the_same_batch_and_lease_until_idempotent_retry() {
    let pool = BudgetPool::new(4096);
    let mut buffer = staged(pool.clone());
    buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
    let expected = buffer.charged_bytes();
    let mut store = Store {
        checkpoint: 10,
        ..Store::default()
    };
    assert_eq!(
        buffer.apply_pending(|batch| {
            store.apply(batch)?;
            Err("ack lost")
        }),
        Err("ack lost")
    );
    assert_eq!(store.checkpoint, 13);
    assert_eq!(*buffer.checkpoint(), 10);
    claims(&pool, expected);
    assert_eq!(
        buffer.pending().unwrap().rows().collect::<Vec<_>>(),
        [b"private\0\xff".as_slice()]
    );
    assert_eq!(
        buffer.apply_pending(|batch| {
            // Shared allocation from a callback must not deadlock on a held lock.
            let other = pool.try_reserve(ResourceKind::Rendering, 1).unwrap();
            store.apply(batch)?;
            drop(other);
            Ok::<(), &'static str>(())
        }),
        Ok(true)
    );
    assert_eq!(store.writes, 1);
    assert_eq!(*buffer.checkpoint(), 13);
    claims(&pool, 0);
}

#[test]
fn async_cancellation_keeps_pending_charge_after_a_durable_commit_then_retries() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let pool = BudgetPool::new(4096);
    let mut buffer = staged(pool.clone());
    buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
    let expected = buffer.charged_bytes();
    let mut store = Store {
        checkpoint: 10,
        ..Store::default()
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut applying = Box::pin(buffer.apply_pending_async(async |batch| {
        store.apply(batch)?;
        let _other = pool.try_reserve(ResourceKind::Rendering, 1).unwrap();
        std::future::pending::<()>().await;
        Ok::<(), &'static str>(())
    }));
    assert!(matches!(
        applying.as_mut().poll(&mut context),
        Poll::Pending
    ));
    claims(&pool, expected);
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        1
    );
    drop(applying);
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Rendering),
        0
    );
    assert_eq!(*buffer.checkpoint(), 10);
    assert_eq!(store.checkpoint, 13);
    claims(&pool, expected);
    let mut retry = Box::pin(buffer.apply_pending_async(async |batch| store.apply(batch)));
    assert!(matches!(
        retry.as_mut().poll(&mut context),
        Poll::Ready(Ok(true))
    ));
    drop(retry);
    assert_eq!(store.writes, 1);
    assert_eq!(*buffer.checkpoint(), 13);
    claims(&pool, 0);
}

#[test]
fn rollback_faults_disconnect_and_drop_release_retained_data_without_advancing_checkpoint() {
    let pool = BudgetPool::new(4096);
    let mut rollback = staged(pool.clone());
    rollback
        .push(Event::Rollback { start: 12, end: 13 })
        .unwrap();
    claims(&pool, 0);
    assert_eq!(rollback.pending().unwrap().rows().len(), 0);
    assert_eq!(*rollback.checkpoint(), 10);
    assert_eq!(rollback.apply_pending(|_| Ok::<(), ()>(())), Ok(true));
    assert_eq!(*rollback.checkpoint(), 13);
    for pending in [false, true] {
        for fault in 0..5 {
            let mut buffer = staged(pool.clone());
            if pending {
                buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
            }
            match fault {
                0 => buffer.disconnect(),
                1 => {
                    let _ = buffer.push(Event::Ddl);
                }
                2 => {
                    let _ = buffer.apply_payload_budget(0);
                }
                3 if !pending => {
                    let _ = buffer.push(Event::Row {
                        start: 99,
                        end: 100,
                        bytes: vec![9],
                    });
                }
                4 if !pending => {
                    let _ = buffer.push(Event::Begin { start: 12, end: 13 });
                }
                _ => {
                    drop(buffer);
                    claims(&pool, 0);
                    continue;
                }
            }
            assert_eq!(*buffer.checkpoint(), 10);
            assert_eq!(buffer.charged_bytes(), 0);
            claims(&pool, 0);
        }
    }
    let zero = BudgetPool::new(0);
    let mut metadata = BudgetedChangeBuffer::new(0, 0, zero.clone());
    metadata.push(Event::Advance { start: 0, end: 1 }).unwrap();
    assert_eq!(metadata.apply_pending(|_| Ok::<(), ()>(())), Ok(true));
    claims(&zero, 0);
}

#[test]
fn a_shared_view_index_and_change_rows_obey_one_pool_and_shrink_debt() {
    let rows = [PostMetaRow {
        meta_id: 0,
        post_id: 0,
        meta_key: Some(b"x".to_vec()),
        meta_value: None,
    }];
    let row_charge = std::mem::size_of::<PostMetaRow>() + 1;
    let index_charge =
        std::mem::size_of::<usize>() + std::mem::size_of::<(Vec<u8>, Vec<usize>)>() + 1;
    let payload = vec![1];
    let raw_charge = charge(&payload);
    let pool = BudgetPool::new(row_charge + index_charge + raw_charge);
    let mut view =
        BudgetedPostMetaView::clone_from_rows(&rows, pool.clone(), 1, row_charge).unwrap();
    view.try_build_key_index(1, index_charge).unwrap();
    let mut buffer = BudgetedChangeBuffer::new(0, usize::MAX, pool.clone());
    buffer.push(Event::Begin { start: 0, end: 1 }).unwrap();
    buffer
        .push(Event::Row {
            start: 1,
            end: 2,
            bytes: payload,
        })
        .unwrap();
    assert_eq!(
        pool.stats().unwrap().claimed,
        row_charge + index_charge + raw_charge
    );
    pool.set_limit(0).unwrap();
    assert_eq!(
        buffer.push(Event::Row {
            start: 2,
            end: 3,
            bytes: vec![2]
        }),
        Err(BudgetedPushError::Budget(BudgetError::Exhausted))
    );
    claims(&pool, 0);
    assert_eq!(pool.stats().unwrap().debt, row_charge + index_charge);
    assert_eq!(view.rows(), rows);
    assert_eq!(view.matching_key(Some(b"x")).count(), 1);
    view.discard_index();
    assert_eq!(pool.stats().unwrap().debt, row_charge);
    drop(view);
    assert_eq!(pool.stats().unwrap().claimed, 0);
}

#[test]
fn discarding_ram_after_lost_ack_does_not_represent_a_durable_rollback() {
    let pool = BudgetPool::new(4096);
    let mut buffer = staged(pool.clone());
    buffer.push(Event::Commit { start: 12, end: 13 }).unwrap();
    let mut store = Store {
        checkpoint: 10,
        ..Store::default()
    };
    assert_eq!(
        buffer.apply_pending(|batch| {
            store.apply(batch)?;
            Err("ack lost")
        }),
        Err("ack lost")
    );
    assert_eq!(
        buffer.apply_payload_budget(0),
        Err(BufferError::RecoveryRequired(Fault::PayloadBudgetExceeded))
    );
    assert_eq!(*buffer.checkpoint(), 10);
    assert_eq!(store.checkpoint, 13);
    assert_eq!(store.writes, 1);
    claims(&pool, 0);
    assert!(buffer.pending().is_none());
}

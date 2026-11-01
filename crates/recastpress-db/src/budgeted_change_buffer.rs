//! Retained row-payload reservations for the existing transaction buffer only.
//!
//! The charge is raw Vec<u8> capacity plus the buffer's existing per-row
//! allowance. Input/decoder allocations, positions, outer-Vec excess capacity,
//! other node/driver allocations, peak memory and RSS are excluded. This is not
//! cross-process coordination, automatic capacity selection or cache policy.
//! Callers must deliberately share the same BudgetPool with other owners.
//!
//! Pending work retains its lease through callback errors, acknowledgement loss
//! and async cancellation. Dropping RAM work never proves a durable rollback:
//! recovery must reconcile durable source/position AND batch identity as before.

use crate::change_buffer::{
    AdmissionError, BufferError, ChangeBuffer, CommittedBatch, Event, Fault, Outcome,
};
use recastpress_core::resource_budget::{BudgetError, BudgetPool, Reservation, ResourceKind};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetedPushError {
    Buffer(BufferError),
    Budget(BudgetError),
}

impl fmt::Display for BudgetedPushError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Buffer(BufferError::ApplyRequired) => "pending transaction requires application",
            Self::Buffer(BufferError::RecoveryRequired(_)) => "change buffer requires recovery",
            Self::Budget(BudgetError::Exhausted) => {
                "retained change budget is exhausted; recovery required"
            }
            Self::Budget(BudgetError::Unavailable) => {
                "retained change budget is unavailable; recovery required"
            }
        })
    }
}

impl std::error::Error for BudgetedPushError {}

/// Move-only. Field order destroys retained data before its optional lease.
/// No Debug, mutable inner reference, Deref or into_inner exposes row ownership.
pub struct BudgetedChangeBuffer<P> {
    inner: ChangeBuffer<P>,
    lease: Option<Reservation>,
    pool: BudgetPool,
}

impl<P: Clone + Eq> BudgetedChangeBuffer<P> {
    /// No row is retained yet. Empty/position-only batches need no reservation.
    pub fn new(checkpoint: P, payload_budget: usize, pool: BudgetPool) -> Self {
        Self {
            inner: ChangeBuffer::new(checkpoint, payload_budget),
            lease: None,
            pool,
        }
    }

    pub fn checkpoint(&self) -> &P {
        self.inner.checkpoint()
    }
    pub fn charged_bytes(&self) -> usize {
        self.inner.charged_bytes()
    }
    pub fn fault(&self) -> Option<Fault> {
        self.inner.fault()
    }
    pub fn pending(&self) -> Option<&CommittedBatch<P>> {
        self.inner.pending()
    }

    pub fn push(&mut self, event: Event<P>) -> Result<Outcome, BudgetedPushError> {
        self.push_using(event, |pool, lease, bytes| match lease {
            Some(lease) => lease.try_resize(bytes),
            None => {
                *lease = Some(pool.try_reserve(ResourceKind::ChangeBuffer, bytes)?);
                Ok(())
            }
        })
    }

    fn push_using(
        &mut self,
        event: Event<P>,
        reserve: impl FnOnce(&BudgetPool, &mut Option<Reservation>, usize) -> Result<(), BudgetError>,
    ) -> Result<Outcome, BudgetedPushError> {
        let pool = &self.pool;
        let lease = &mut self.lease;
        let result = self
            .inner
            .push_with_admission(event, |bytes| reserve(pool, lease, bytes));
        self.release_if_empty();
        result.map_err(|error| match error {
            AdmissionError::Buffer(error) => BudgetedPushError::Buffer(error),
            AdmissionError::Admission(error) => BudgetedPushError::Budget(error),
        })
    }

    fn release_if_empty(&mut self) {
        if self.inner.charged_bytes() == 0 {
            // Data has already been destroyed. Drop is deliberately infallible:
            // poison conservatively retains ledger charges, never turns an
            // acknowledged durable commit into an application error.
            drop(self.lease.take());
        }
    }

    pub fn apply_payload_budget(&mut self, bytes: usize) -> Result<(), BufferError> {
        let result = self.inner.apply_payload_budget(bytes);
        self.release_if_empty();
        result
    }

    pub fn disconnect(&mut self) {
        self.inner.disconnect();
        self.release_if_empty();
    }

    /// Callback errors keep the pending batch and reservation for retry. A
    /// callback's durable effects are not undone by this buffer or by Drop.
    pub fn apply_pending<E>(
        &mut self,
        atomic_apply: impl FnOnce(&CommittedBatch<P>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let result = self.inner.apply_pending(atomic_apply);
        self.release_if_empty();
        result
    }

    /// Cancellation retains the same pending data and lease. No budget mutex
    /// remains held while awaiting the caller's storage transaction.
    pub async fn apply_pending_async<E>(
        &mut self,
        atomic_apply: impl std::ops::AsyncFnOnce(&CommittedBatch<P>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let result = self.inner.apply_pending_async(atomic_apply).await;
        self.release_if_empty();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_admission_discards_the_whole_transaction_and_latches_recovery() {
        let pool = BudgetPool::new(1024);
        let mut buffer = BudgetedChangeBuffer::new(0, 1024, pool.clone());
        buffer.push(Event::Begin { start: 0, end: 1 }).unwrap();
        buffer
            .push(Event::Row {
                start: 1,
                end: 2,
                bytes: vec![7],
            })
            .unwrap();
        assert!(pool.stats().unwrap().claimed > 0);
        let result = buffer.push_using(
            Event::Row {
                start: 2,
                end: 3,
                bytes: vec![8],
            },
            |_, _, _| Err(BudgetError::Unavailable),
        );
        assert_eq!(
            result,
            Err(BudgetedPushError::Budget(BudgetError::Unavailable))
        );
        assert_eq!(buffer.fault(), Some(Fault::PayloadBudgetExceeded));
        assert_eq!(buffer.charged_bytes(), 0);
        assert_eq!(*buffer.checkpoint(), 0);
        assert_eq!(pool.stats().unwrap().claimed, 0);
        assert_eq!(
            buffer.push(Event::Ddl),
            Err(BudgetedPushError::Buffer(BufferError::RecoveryRequired(
                Fault::PayloadBudgetExceeded
            )))
        );
    }
}

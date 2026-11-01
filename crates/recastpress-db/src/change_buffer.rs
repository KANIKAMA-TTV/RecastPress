//! Transaction staging for a future trusted binlog decoder (R60/R61).
//!
//! This is not a wire decoder, subscriber, snapshot coordinator or durable store.
//! Position equality must include validated source identity and log coordinates;
//! this module deliberately does not compare MySQL/MariaDB position strings.
//! The decoder must validate forward movement, rotations and event integrity;
//! equality checking here alone is not a proof of monotonic source positions.

/// Opaque, already classified events from a trusted decoder. Row bytes remain
/// editing-side data and must never be sent directly to a public reader.
pub enum Event<P> {
    Begin {
        start: P,
        end: P,
    },
    Row {
        start: P,
        end: P,
        bytes: Vec<u8>,
    },
    Commit {
        start: P,
        end: P,
    },
    Rollback {
        start: P,
        end: P,
    },
    /// Non-mutating events. Within a transaction these only advance its cursor.
    /// Outside a transaction the decoder must prove this is an independently
    /// resumable boundary, with all decoder context available after restart.
    /// GTID/transaction-start metadata belongs to Begin, never an idle Advance.
    /// A decoder must never classify an unknown event or DDL as Advance.
    Advance {
        start: P,
        end: P,
    },
    Ddl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    PositionGap,
    UnexpectedEvent,
    PayloadBudgetExceeded,
    SchemaChanged,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferError {
    RecoveryRequired(Fault),
    ApplyRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Staged,
    ApplyRequired,
}

pub(super) enum AdmissionError<E> {
    Buffer(BufferError),
    Admission(E),
}

/// A complete transaction or a position-only batch. There is intentionally no
/// Debug implementation: row payloads may contain passwords or private posts.
pub struct CommittedBatch<P> {
    from: P,
    through: P,
    rows: Vec<Vec<u8>>,
    charged: usize,
}

impl<P> CommittedBatch<P> {
    pub fn from(&self) -> &P {
        &self.from
    }
    pub fn through(&self) -> &P {
        &self.through
    }
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.rows.iter().map(Vec::as_slice)
    }
    pub fn charged_bytes(&self) -> usize {
        self.charged
    }
}

enum State<P> {
    Idle,
    Transaction { rows: Vec<Vec<u8>>, charged: usize },
    Pending(CommittedBatch<P>),
    Broken(Fault),
}

/// The budget covers retained row capacities plus a per-row allocation allowance;
/// it is not a total-process/RSS bound. A future allocator must also reserve
/// decoder, position, index-construction and cache memory without double spending.
pub struct ChangeBuffer<P> {
    checkpoint: P,
    cursor: P,
    budget: usize,
    state: State<P>,
}

impl<P: Clone + Eq> ChangeBuffer<P> {
    /// The supplied checkpoint must already be durably paired with the internal
    /// data snapshot by the host. Constructing this object does not prove that.
    pub fn new(checkpoint: P, payload_budget: usize) -> Self {
        Self {
            cursor: checkpoint.clone(),
            checkpoint,
            budget: payload_budget,
            state: State::Idle,
        }
    }

    pub fn checkpoint(&self) -> &P {
        &self.checkpoint
    }

    pub fn charged_bytes(&self) -> usize {
        match &self.state {
            State::Transaction { charged, .. } => *charged,
            State::Pending(batch) => batch.charged,
            _ => 0,
        }
    }

    /// A trusted shared allocator may reduce this allocation. If retained work
    /// no longer fits, discard it and recover from the durable checkpoint; never
    /// silently drop individual rows and commit an incomplete transaction.
    pub fn apply_payload_budget(&mut self, bytes: usize) -> Result<(), BufferError> {
        self.budget = bytes;
        if let State::Broken(fault) = self.state {
            return Err(BufferError::RecoveryRequired(fault));
        }
        if self.charged_bytes() > bytes {
            self.state = State::Broken(Fault::PayloadBudgetExceeded);
            return Err(BufferError::RecoveryRequired(Fault::PayloadBudgetExceeded));
        }
        Ok(())
    }

    pub fn fault(&self) -> Option<Fault> {
        match self.state {
            State::Broken(fault) => Some(fault),
            _ => None,
        }
    }

    pub fn pending(&self) -> Option<&CommittedBatch<P>> {
        match &self.state {
            State::Pending(batch) => Some(batch),
            _ => None,
        }
    }

    /// Apply data and persist `through` in ONE atomic host transaction. The host
    /// must make retries idempotent for the same source/batch identity and reject
    /// conflicting content at the same position. A lost commit acknowledgement
    /// may mean durable state advanced even when the callback reports an error.
    /// Success alone advances this in-memory checkpoint. Errors keep the batch
    /// for retry and backpressure the decoder; external effects are not undone.
    pub fn apply_pending<E>(
        &mut self,
        atomic_apply: impl FnOnce(&CommittedBatch<P>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let State::Pending(batch) = &self.state else {
            return Ok(false);
        };
        atomic_apply(batch)?;
        self.checkpoint = batch.through.clone();
        self.state = State::Idle;
        Ok(true)
    }

    /// Async form of apply_pending, allowing an actual storage transaction to
    /// be awaited without blocking an executor. Cancellation leaves the batch
    /// pending, but the durable commit may already have happened. The host must
    /// check durable source/position AND batch identity when retrying, and reload
    /// durable state before recovery; this buffer is not the durable authority.
    pub async fn apply_pending_async<E>(
        &mut self,
        atomic_apply: impl std::ops::AsyncFnOnce(&CommittedBatch<P>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let State::Pending(batch) = &self.state else {
            return Ok(false);
        };
        atomic_apply(batch).await?;
        self.checkpoint = batch.through.clone();
        self.state = State::Idle;
        Ok(true)
    }

    /// Disconnect invalidates pending work as well as a partial transaction. The
    /// host must also purge/stop response caches and reconcile before restarting.
    pub fn disconnect(&mut self) {
        if !matches!(self.state, State::Broken(_)) {
            self.state = State::Broken(Fault::Disconnected);
        }
    }

    pub fn push(&mut self, event: Event<P>) -> Result<Outcome, BufferError> {
        match self.push_with_admission(event, |_| Ok::<(), std::convert::Infallible>(())) {
            Ok(outcome) => Ok(outcome),
            Err(AdmissionError::Buffer(error)) => Err(error),
            Err(AdmissionError::Admission(never)) => match never {},
        }
    }

    /// Private admission runs only after the existing structural/local checks,
    /// before retaining the Row. Its caller must hold no allocator lock here.
    pub(super) fn push_with_admission<E>(
        &mut self,
        event: Event<P>,
        admit_row: impl FnOnce(usize) -> Result<(), E>,
    ) -> Result<Outcome, AdmissionError<E>> {
        if let State::Broken(fault) = self.state {
            return Err(AdmissionError::Buffer(BufferError::RecoveryRequired(fault)));
        }
        // DDL invalidates even a completed batch not yet applied. It must never
        // be hidden by pending-batch backpressure.
        if matches!(event, Event::Ddl) {
            return self
                .fail(Fault::SchemaChanged)
                .map_err(AdmissionError::Buffer);
        }
        if matches!(self.state, State::Pending(_)) {
            return Err(AdmissionError::Buffer(BufferError::ApplyRequired));
        }
        let (start, end) = match &event {
            Event::Begin { start, end }
            | Event::Row { start, end, .. }
            | Event::Commit { start, end }
            | Event::Rollback { start, end }
            | Event::Advance { start, end } => (start, end),
            Event::Ddl => unreachable!(),
        };
        if *start != self.cursor || start == end {
            return self
                .fail(Fault::PositionGap)
                .map_err(AdmissionError::Buffer);
        }
        let next = end.clone();
        let state = std::mem::replace(&mut self.state, State::Idle);
        let outcome = match (state, event) {
            (State::Idle, Event::Begin { .. }) => {
                self.state = State::Transaction {
                    rows: Vec::new(),
                    charged: 0,
                };
                Outcome::Staged
            }
            (State::Transaction { mut rows, charged }, Event::Row { bytes, .. }) => {
                let charge = bytes
                    .capacity()
                    .checked_add(2 * std::mem::size_of::<Vec<u8>>())
                    .and_then(|charge| charged.checked_add(charge));
                let Some(charged) = charge.filter(|charge| *charge <= self.budget) else {
                    return self
                        .fail(Fault::PayloadBudgetExceeded)
                        .map_err(AdmissionError::Buffer);
                };
                if let Err(error) = admit_row(charged) {
                    // The wrapper may release its old lease only after this
                    // function returns: discard both old and rejected data now.
                    drop(rows);
                    drop(bytes);
                    let _ = self.fail(Fault::PayloadBudgetExceeded);
                    return Err(AdmissionError::Admission(error));
                }
                rows.push(bytes);
                self.state = State::Transaction { rows, charged };
                Outcome::Staged
            }
            (State::Transaction { rows, charged }, Event::Commit { .. }) => {
                self.state = State::Pending(CommittedBatch {
                    from: self.checkpoint.clone(),
                    through: next.clone(),
                    rows,
                    charged,
                });
                Outcome::ApplyRequired
            }
            (State::Transaction { .. }, Event::Rollback { .. })
            | (State::Idle, Event::Advance { .. }) => {
                self.state = State::Pending(CommittedBatch {
                    from: self.checkpoint.clone(),
                    through: next.clone(),
                    rows: Vec::new(),
                    charged: 0,
                });
                Outcome::ApplyRequired
            }
            (state @ State::Transaction { .. }, Event::Advance { .. }) => {
                self.state = state;
                Outcome::Staged
            }
            _ => {
                return self
                    .fail(Fault::UnexpectedEvent)
                    .map_err(AdmissionError::Buffer);
            }
        };
        self.cursor = next;
        Ok(outcome)
    }

    fn fail(&mut self, fault: Fault) -> Result<Outcome, BufferError> {
        self.state = State::Broken(fault);
        Err(BufferError::RecoveryRequired(fault))
    }
}

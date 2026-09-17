//! Public upload lifecycle values and telemetry.

use thiserror::Error;

/// Monotonic submission completion value supplied by a backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FenceValue(pub u64);

/// Stable identifier for one ring reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UploadTicket(pub(super) u64);

impl UploadTicket {
    /// Monotonic ticket sequence, useful for tracing.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

/// Current lifecycle of an upload reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadState {
    /// Reserved bytes may be filled by the caller.
    Reserved,
    /// Staging bytes are complete and may be submitted.
    Ready,
    /// A backend submission may still read the bytes.
    InFlight(FenceValue),
    /// The reservation was cancelled before submission.
    Cancelled,
}

/// Immutable description returned when bytes are reserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadReservation {
    pub(super) ticket: UploadTicket,
    pub(super) offset: usize,
    pub(super) len: usize,
}

impl UploadReservation {
    /// Ticket used for commit, submission and cancellation.
    #[must_use]
    pub const fn ticket(self) -> UploadTicket {
        self.ticket
    }

    /// Offset into the ring's staging buffer.
    #[must_use]
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Payload byte length.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len
    }

    /// Whether the payload is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// Fixed resource and upload throughput budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadRingConfig {
    /// Staging bytes retained for the lifetime of the ring.
    pub capacity_bytes: usize,
    /// Maximum simultaneous reservations.
    pub ticket_capacity: usize,
    /// Maximum payload bytes accepted between calls to `begin_epoch`.
    pub epoch_budget_bytes: usize,
    /// Maximum submitted payload bytes awaiting fence completion.
    pub in_flight_budget_bytes: usize,
    /// Required payload offset alignment. Must be a power of two.
    pub alignment: usize,
}

impl UploadRingConfig {
    /// Validates capacities and budgets without allocating staging storage.
    ///
    /// # Errors
    ///
    /// Rejects empty capacities or budgets, an in-flight limit larger than
    /// staging capacity, and alignment that is not a power of two.
    pub const fn validate(self) -> Result<(), UploadBackpressure> {
        if self.capacity_bytes == 0
            || self.ticket_capacity == 0
            || self.epoch_budget_bytes == 0
            || self.in_flight_budget_bytes == 0
            || self.in_flight_budget_bytes > self.capacity_bytes
            || !self.alignment.is_power_of_two()
        {
            return Err(UploadBackpressure::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Observable ring counters. Values are cumulative except active gauges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadMetrics {
    /// Payload bytes committed into staging.
    pub bytes_staged: u64,
    /// Payload bytes handed to backend submissions.
    pub bytes_submitted: u64,
    /// Payload bytes made reusable after fence completion.
    pub bytes_retired: u64,
    /// Payload bytes cancelled before submission.
    pub bytes_cancelled: u64,
    /// Payload bytes accepted in the current epoch.
    pub epoch_bytes: u64,
    /// Submitted payload bytes awaiting fence completion.
    pub in_flight_bytes: u64,
    /// Largest in-flight payload footprint observed.
    pub peak_in_flight_bytes: u64,
    /// Ring span occupied by live reservations, including padding.
    pub occupied_bytes: u64,
    /// Largest occupied ring span observed.
    pub peak_occupied_bytes: u64,
    /// Number of live reservations.
    pub active_tickets: u64,
    /// Requests rejected by any budget or capacity bound.
    pub stall_events: u64,
    /// Payload bytes represented by rejected requests.
    pub stalled_bytes: u64,
    /// Host storage allocations performed by this primitive.
    pub host_allocation_events: u64,
}

/// Why an upload reservation or lifecycle transition could not proceed.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum UploadBackpressure {
    /// Configuration must provide non-zero capacities and power-of-two alignment.
    #[error("upload ring configuration is invalid")]
    InvalidConfiguration,
    /// Empty uploads are rejected.
    #[error("an upload reservation must contain at least one byte")]
    EmptyUpload,
    /// The configured per-epoch payload budget was exhausted.
    #[error("upload epoch budget exhausted")]
    EpochBudget,
    /// Submitted work reached its fence-protected byte budget.
    #[error("upload in-flight budget exhausted")]
    InFlightBudget,
    /// Staging bytes remain occupied by older work.
    #[error("upload staging ring is full")]
    RingFull,
    /// All ticket records are in use.
    #[error("upload ticket capacity is exhausted")]
    TicketCapacity,
    /// Arithmetic could not represent the reservation.
    #[error("upload reservation size overflow")]
    SizeOverflow,
    /// The ticket is stale or in the wrong lifecycle state.
    #[error("upload ticket is stale or has an invalid state")]
    InvalidTicket,
}

/// Work made reusable by one retirement call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Retirement {
    /// Number of retired tickets.
    pub tickets: usize,
    /// Payload bytes released after completed submissions.
    pub bytes: u64,
}

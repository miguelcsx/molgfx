//! Fixed-capacity upload staging with explicit backpressure and fence-safe
//! retirement.
//!
//! Reservations advance monotonically through a byte ring and retire in FIFO
//! order. Alignment and wrap padding remain occupied until the reservation's
//! fence completes, preventing reuse while a backend may still read it.

use super::upload_types::{
    FenceValue, Retirement, UploadBackpressure, UploadMetrics, UploadReservation, UploadRingConfig,
    UploadState, UploadTicket,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordState {
    Vacant,
    Live(UploadState),
}

#[derive(Clone, Copy, Debug)]
struct UploadRecord {
    ticket: UploadTicket,
    payload_start: u64,
    end: u64,
    payload_len: usize,
    state: RecordState,
}

impl UploadRecord {
    const fn vacant() -> Self {
        Self {
            ticket: UploadTicket(0),
            payload_start: 0,
            end: 0,
            payload_len: 0,
            state: RecordState::Vacant,
        }
    }
}

/// A bounded host staging ring independent of any concrete GPU backend.
#[derive(Debug)]
pub struct UploadRing {
    bytes: Vec<u8>,
    records: Vec<UploadRecord>,
    config: UploadRingConfig,
    head: usize,
    active: usize,
    write_cursor: u64,
    retired_cursor: u64,
    next_ticket: u64,
    metrics: UploadMetrics,
}

impl UploadRing {
    /// Allocates byte and ticket storage once.
    ///
    /// # Errors
    ///
    /// Returns [`UploadBackpressure::InvalidConfiguration`] for empty
    /// capacities, zero budget or non-power-of-two alignment.
    pub fn new(config: UploadRingConfig) -> Result<Self, UploadBackpressure> {
        config.validate()?;
        Ok(Self {
            bytes: vec![0; config.capacity_bytes],
            records: vec![UploadRecord::vacant(); config.ticket_capacity],
            config,
            head: 0,
            active: 0,
            write_cursor: 0,
            retired_cursor: 0,
            next_ticket: 1,
            metrics: UploadMetrics {
                host_allocation_events: 2,
                ..UploadMetrics::default()
            },
        })
    }

    /// Resets only the per-epoch payload counter.
    pub fn begin_epoch(&mut self) {
        self.metrics.epoch_bytes = 0;
    }

    /// Reserves one contiguous aligned staging range.
    ///
    /// # Errors
    ///
    /// Returns explicit backpressure without growing either storage vector.
    pub fn reserve(&mut self, byte_len: usize) -> Result<UploadReservation, UploadBackpressure> {
        if byte_len == 0 {
            return Err(UploadBackpressure::EmptyUpload);
        }
        if self.metrics.epoch_bytes.saturating_add(byte_len as u64)
            > self.config.epoch_budget_bytes as u64
        {
            return self.stall(byte_len, UploadBackpressure::EpochBudget);
        }
        if self.active == self.records.len() {
            return self.stall(byte_len, UploadBackpressure::TicketCapacity);
        }
        if self.active == 0 {
            self.write_cursor = 0;
            self.retired_cursor = 0;
        }
        let (payload_start, end) = self.extent_for(byte_len)?;
        if end - self.retired_cursor > self.bytes.len() as u64 {
            return self.stall(byte_len, UploadBackpressure::RingFull);
        }
        let offset = self.byte_offset(payload_start)?;
        let ticket = UploadTicket(self.next_ticket);
        self.next_ticket = self.next_ticket.wrapping_add(1).max(1);
        let index = (self.head + self.active) % self.records.len();
        self.records[index] = UploadRecord {
            ticket,
            payload_start,
            end,
            payload_len: byte_len,
            state: RecordState::Live(UploadState::Reserved),
        };
        self.active += 1;
        self.write_cursor = end;
        self.metrics.epoch_bytes = self.metrics.epoch_bytes.saturating_add(byte_len as u64);
        self.refresh_gauges();
        Ok(UploadReservation {
            ticket,
            offset,
            len: byte_len,
        })
    }

    /// Returns the writable payload range while a ticket is reserved.
    ///
    /// # Errors
    ///
    /// Returns [`UploadBackpressure::InvalidTicket`] for stale or already
    /// committed reservations.
    pub fn bytes_mut(
        &mut self,
        reservation: UploadReservation,
    ) -> Result<&mut [u8], UploadBackpressure> {
        let index = self.record_index(reservation.ticket, UploadState::Reserved)?;
        let record = self.records[index];
        let expected_offset = self.byte_offset(record.payload_start)?;
        if record.payload_len != reservation.len || expected_offset != reservation.offset {
            return Err(UploadBackpressure::InvalidTicket);
        }
        let start = reservation.offset;
        let end = start + reservation.len;
        Ok(&mut self.bytes[start..end])
    }

    /// Marks filled bytes ready for backend submission.
    ///
    /// # Errors
    ///
    /// The ticket must currently be reserved.
    pub fn commit(&mut self, ticket: UploadTicket) -> Result<(), UploadBackpressure> {
        let index = self.record_index(ticket, UploadState::Reserved)?;
        self.records[index].state = RecordState::Live(UploadState::Ready);
        self.metrics.bytes_staged = self
            .metrics
            .bytes_staged
            .saturating_add(self.records[index].payload_len as u64);
        Ok(())
    }

    /// Associates a ready upload with the fence of the submission reading it.
    ///
    /// # Errors
    ///
    /// The ticket must currently be ready.
    pub fn submit(
        &mut self,
        ticket: UploadTicket,
        fence: FenceValue,
    ) -> Result<(), UploadBackpressure> {
        let index = self.record_index(ticket, UploadState::Ready)?;
        let payload_len = self.records[index].payload_len as u64;
        if self.metrics.in_flight_bytes.saturating_add(payload_len)
            > self.config.in_flight_budget_bytes as u64
        {
            return self.stall(
                self.records[index].payload_len,
                UploadBackpressure::InFlightBudget,
            );
        }
        self.records[index].state = RecordState::Live(UploadState::InFlight(fence));
        self.metrics.bytes_submitted = self.metrics.bytes_submitted.saturating_add(payload_len);
        self.metrics.in_flight_bytes = self.metrics.in_flight_bytes.saturating_add(payload_len);
        self.metrics.peak_in_flight_bytes = self
            .metrics
            .peak_in_flight_bytes
            .max(self.metrics.in_flight_bytes);
        Ok(())
    }

    /// Validates in-flight admission before a backend submission is issued.
    ///
    /// # Errors
    ///
    /// The ticket must be ready and fit the fixed in-flight byte budget.
    pub fn ensure_submittable(&mut self, ticket: UploadTicket) -> Result<(), UploadBackpressure> {
        let index = self.record_index(ticket, UploadState::Ready)?;
        let payload_len = self.records[index].payload_len as u64;
        if self.metrics.in_flight_bytes.saturating_add(payload_len)
            > self.config.in_flight_budget_bytes as u64
        {
            return self.stall(
                self.records[index].payload_len,
                UploadBackpressure::InFlightBudget,
            );
        }
        Ok(())
    }

    /// Cancels a reservation that has not been submitted.
    ///
    /// # Errors
    ///
    /// In-flight and stale tickets cannot be cancelled.
    pub fn cancel(&mut self, ticket: UploadTicket) -> Result<(), UploadBackpressure> {
        let Some(index) = self.find_ticket(ticket) else {
            return Err(UploadBackpressure::InvalidTicket);
        };
        match self.records[index].state {
            RecordState::Live(UploadState::Reserved | UploadState::Ready) => {
                self.records[index].state = RecordState::Live(UploadState::Cancelled);
                self.metrics.bytes_cancelled = self
                    .metrics
                    .bytes_cancelled
                    .saturating_add(self.records[index].payload_len as u64);
                self.reclaim_cancelled_front();
                Ok(())
            }
            RecordState::Vacant
            | RecordState::Live(UploadState::InFlight(_) | UploadState::Cancelled) => {
                Err(UploadBackpressure::InvalidTicket)
            }
        }
    }

    /// Retires the contiguous FIFO prefix whose submission fences completed.
    #[must_use]
    pub fn retire(&mut self, completed: FenceValue) -> Retirement {
        let mut retirement = Retirement::default();
        while self.active > 0 {
            let record = self.records[self.head];
            let completed_record = matches!(
                record.state,
                RecordState::Live(UploadState::InFlight(fence)) if fence <= completed
            );
            if !completed_record {
                break;
            }
            retirement.tickets = retirement.tickets.saturating_add(1);
            retirement.bytes = retirement.bytes.saturating_add(record.payload_len as u64);
            self.metrics.bytes_retired = self
                .metrics
                .bytes_retired
                .saturating_add(record.payload_len as u64);
            self.metrics.in_flight_bytes = self
                .metrics
                .in_flight_bytes
                .saturating_sub(record.payload_len as u64);
            self.reclaim_head(record.end);
        }
        retirement
    }

    /// Current lifecycle for a live ticket.
    #[must_use]
    pub fn state(&self, ticket: UploadTicket) -> Option<UploadState> {
        let index = self.find_ticket(ticket)?;
        match self.records[index].state {
            RecordState::Live(state) => Some(state),
            RecordState::Vacant => None,
        }
    }

    /// Current cumulative counters.
    #[must_use]
    pub const fn metrics(&self) -> UploadMetrics {
        self.metrics
    }

    /// Entire staging storage for backend copy commands.
    #[must_use]
    pub fn staging_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn extent_for(&self, byte_len: usize) -> Result<(u64, u64), UploadBackpressure> {
        let alignment = self.config.alignment as u64;
        let mask = alignment - 1;
        let begin = self.write_cursor;
        let mut payload_start = begin
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or(UploadBackpressure::SizeOverflow)?;
        let offset = payload_start % self.bytes.len() as u64;
        if offset + byte_len as u64 > self.bytes.len() as u64 {
            payload_start = payload_start
                .checked_add(self.bytes.len() as u64 - offset)
                .ok_or(UploadBackpressure::SizeOverflow)?;
        }
        let end = payload_start
            .checked_add(byte_len as u64)
            .ok_or(UploadBackpressure::SizeOverflow)?;
        Ok((payload_start, end))
    }

    fn record_index(
        &self,
        ticket: UploadTicket,
        expected: UploadState,
    ) -> Result<usize, UploadBackpressure> {
        let Some(index) = self.find_ticket(ticket) else {
            return Err(UploadBackpressure::InvalidTicket);
        };
        if self.records[index].state != RecordState::Live(expected) {
            return Err(UploadBackpressure::InvalidTicket);
        }
        Ok(index)
    }

    fn find_ticket(&self, ticket: UploadTicket) -> Option<usize> {
        if ticket.0 == 0 || self.records.is_empty() {
            return None;
        }
        let Ok(slot_count) = u64::try_from(self.records.len()) else {
            return None;
        };
        let slot = (ticket.0 - 1) % slot_count;
        let Ok(index) = usize::try_from(slot) else {
            return None;
        };
        (self.records[index].ticket == ticket
            && !matches!(self.records[index].state, RecordState::Vacant))
        .then_some(index)
    }

    fn byte_offset(&self, cursor: u64) -> Result<usize, UploadBackpressure> {
        let capacity =
            u64::try_from(self.bytes.len()).map_err(|_| UploadBackpressure::SizeOverflow)?;
        usize::try_from(cursor % capacity).map_err(|_| UploadBackpressure::SizeOverflow)
    }

    fn reclaim_cancelled_front(&mut self) {
        while self.active > 0 {
            let record = self.records[self.head];
            if record.state != RecordState::Live(UploadState::Cancelled) {
                break;
            }
            self.reclaim_head(record.end);
        }
    }

    fn reclaim_head(&mut self, end: u64) {
        self.records[self.head] = UploadRecord::vacant();
        self.retired_cursor = end;
        self.head = (self.head + 1) % self.records.len();
        self.active -= 1;
        self.refresh_gauges();
    }

    fn refresh_gauges(&mut self) {
        self.metrics.active_tickets = self.active as u64;
        self.metrics.occupied_bytes = self.write_cursor - self.retired_cursor;
        self.metrics.peak_occupied_bytes = self
            .metrics
            .peak_occupied_bytes
            .max(self.metrics.occupied_bytes);
    }

    fn stall<T>(
        &mut self,
        byte_len: usize,
        error: UploadBackpressure,
    ) -> Result<T, UploadBackpressure> {
        self.metrics.stall_events = self.metrics.stall_events.saturating_add(1);
        self.metrics.stalled_bytes = self.metrics.stalled_bytes.saturating_add(byte_len as u64);
        Err(error)
    }
}

#[cfg(test)]
#[path = "upload_tests.rs"]
mod tests;

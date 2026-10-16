//! Picks that ride the frame loop: requested at any time, resolved as the
//! embedder polls, never blocking and never allocating per pick.
//!
//! A fixed ring of readback slots serves every stream. Each [`Engine::poll`]
//! finishes the maps the device completed, then records every queued request
//! into free slots with one encoder and one submission.

use super::picking::{PICK_READBACK_BYTES, Pick, encode_pick_copies, resolve_packed_pick};
use crate::engine::Engine;
use crate::error::RenderError;
use molgfx_core::{PickPageTicket, PickingError};
use molgfx_gpu::{BufferDesc, BufferUsage, Device, PollOutcome, Queue as _, Readback as _};

/// Bytes one resolved pick occupies, held beside its slot.
const SCRATCH_BYTES: usize = 1024;
const _: () = assert!(SCRATCH_BYTES as u64 == PICK_READBACK_BYTES);

/// Readback slots shared by every stream.
const SLOTS: usize = 4;

/// Resolved picks held for the caller before polling stops finishing maps.
const READY_CAPACITY: usize = SLOTS * 2;

/// Requests an [`PickPolicy::Ordered`] stream can hold before it refuses.
const ORDERED_CAPACITY: usize = 4;

/// What a stream does with a request made while another is outstanding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickPolicy {
    /// Only the newest request matters: an older one still waiting is replaced
    /// and an older result is dropped when a newer one exists.
    LatestWins,
    /// Every request is answered in order; a full queue refuses instead.
    Ordered,
}

/// An independent source of picks, each with its own policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickStream {
    /// Pointer motion: only the newest position matters.
    Hover,
    /// A click or tap: every one is answered, in order.
    Activate,
}

impl PickStream {
    /// How this stream treats overlapping requests.
    #[must_use]
    pub const fn policy(self) -> PickPolicy {
        match self {
            Self::Hover => PickPolicy::LatestWins,
            Self::Activate => PickPolicy::Ordered,
        }
    }
}

/// One resolved pick and the pixel it answers.
#[derive(Debug)]
pub struct PickUpdate {
    /// The stream that asked.
    pub stream: PickStream,
    /// Requested pixel column, top-left origin.
    pub x: u32,
    /// Requested pixel row, top-left origin.
    pub y: u32,
    /// The visible entity, `None` over the background or outside the target.
    pub pick: Result<Option<Pick>, RenderError>,
}

/// What one [`Engine::poll`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PollReport {
    /// Whether the device still holds submitted work.
    pub outcome: PollOutcome,
    /// Picks resolved by this poll and waiting for [`Engine::take_picks`].
    pub picks_resolved: usize,
}

#[derive(Clone, Copy, Debug)]
struct Flight {
    stream: PickStream,
    x: u32,
    y: u32,
    sequence: u64,
    mapped: bool,
}

struct Slot<D: Device> {
    buffer: D::Buffer,
    readback: D::Readback,
    pages: Box<[Option<PickPageTicket>]>,
    scratch: [u8; SCRATCH_BYTES],
    flight: Option<Flight>,
}

/// The ring, the waiting requests and the resolved-but-untaken results.
pub(crate) struct StreamPicker<D: Device> {
    slots: Box<[Slot<D>]>,
    hover: Option<(u32, u32)>,
    ordered: [(u32, u32); ORDERED_CAPACITY],
    ordered_head: usize,
    ordered_len: usize,
    ready: Vec<PickUpdate>,
    sequence: u64,
    hover_delivered: u64,
}

impl<D: Device> std::fmt::Debug for StreamPicker<D> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StreamPicker")
            .field("slots", &self.slots.len())
            .field("ready", &self.ready.len())
            .finish_non_exhaustive()
    }
}

impl<D: Device> StreamPicker<D> {
    pub(crate) fn new(device: &D, queue: &D::Queue) -> Result<Self, RenderError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(SLOTS)
            .map_err(|_| PickingError::AllocationFailed)?;
        for _ in 0..SLOTS {
            let buffer = device.create_buffer(&BufferDesc {
                label: "stream pick readback",
                size: PICK_READBACK_BYTES,
                usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
            })?;
            let readback = queue.readback(device, &buffer);
            slots.push(Slot {
                buffer,
                readback,
                pages: Box::default(),
                scratch: [0; SCRATCH_BYTES],
                flight: None,
            });
        }
        let mut ready = Vec::new();
        ready
            .try_reserve_exact(READY_CAPACITY)
            .map_err(|_| PickingError::AllocationFailed)?;
        Ok(Self {
            slots: slots.into_boxed_slice(),
            hover: None,
            ordered: [(0, 0); ORDERED_CAPACITY],
            ordered_head: 0,
            ordered_len: 0,
            ready,
            sequence: 0,
            hover_delivered: 0,
        })
    }

    fn request(&mut self, stream: PickStream, x: u32, y: u32) -> Result<(), PickingError> {
        match stream.policy() {
            PickPolicy::LatestWins => self.hover = Some((x, y)),
            PickPolicy::Ordered => {
                if self.ordered_len == ORDERED_CAPACITY {
                    return Err(PickingError::StreamBusy);
                }
                let tail = (self.ordered_head + self.ordered_len) % ORDERED_CAPACITY;
                self.ordered[tail] = (x, y);
                self.ordered_len += 1;
            }
        }
        Ok(())
    }

    /// Drops what has not been recorded yet: positions that name an old size.
    pub(crate) fn clear_requests(&mut self) {
        self.hover = None;
        self.ordered_len = 0;
    }

    /// The next request to record: ordered activations first.
    fn next_request(&mut self) -> Option<(PickStream, u32, u32)> {
        if self.ordered_len > 0 {
            let (x, y) = self.ordered[self.ordered_head];
            self.ordered_head = (self.ordered_head + 1) % ORDERED_CAPACITY;
            self.ordered_len -= 1;
            return Some((PickStream::Activate, x, y));
        }
        let (x, y) = self.hover.take()?;
        Some((PickStream::Hover, x, y))
    }

    fn has_room(&self) -> bool {
        self.ready.len() < READY_CAPACITY
    }

    fn push(&mut self, update: PickUpdate, sequence: u64) {
        if update.stream == PickStream::Hover {
            if sequence < self.hover_delivered {
                return;
            }
            self.hover_delivered = sequence;
            self.ready
                .retain(|queued| queued.stream != PickStream::Hover);
        }
        self.ready.push(update);
    }
}

impl<D: Device> Engine<D> {
    /// Asks for the visible entity at one top-left-origin pixel.
    ///
    /// Returns at once and allocates nothing. The answer arrives through
    /// [`Self::take_picks`] after later [`Self::poll`] calls. A
    /// [`PickPolicy::LatestWins`] stream replaces an unrecorded request.
    ///
    /// # Errors
    ///
    /// Returns [`PickingError::StreamBusy`] when an ordered stream already
    /// holds as many requests as it can.
    pub fn request_pick(&mut self, stream: PickStream, x: u32, y: u32) -> Result<(), RenderError> {
        Ok(self.streams.request(stream, x, y)?)
    }

    /// Hands over every resolved pick, oldest first; dropping the iterator
    /// discards what it did not yield.
    pub fn take_picks(&mut self) -> std::vec::Drain<'_, PickUpdate> {
        self.streams.ready.drain(..)
    }

    /// Drives the device and the pick ring without blocking.
    ///
    /// Call once per frame, or from whatever loop owns the thread. Completed
    /// readbacks become [`PickUpdate`]s, and requests made since the last call
    /// are recorded into free slots under a single submission.
    ///
    /// # Errors
    ///
    /// Returns a typed device error, including a sticky device loss.
    pub fn poll(&mut self) -> Result<PollReport, RenderError> {
        let outcome = self.device.poll()?;
        let before = self.streams.ready.len();
        self.finish_streams()?;
        self.dispatch_streams()?;
        Ok(PollReport {
            outcome,
            picks_resolved: self.streams.ready.len().saturating_sub(before),
        })
    }

    fn finish_streams(&mut self) -> Result<(), RenderError> {
        for index in 0..self.streams.slots.len() {
            if !self.streams.has_room() {
                break;
            }
            let slot = &mut self.streams.slots[index];
            let Some(flight) = slot.flight else { continue };
            if !flight.mapped || !slot.readback.try_finish_into(&mut slot.scratch)? {
                continue;
            }
            slot.flight = None;
            let pick = resolve_packed_pick(&self.scene_gpu, &slot.scratch, &slot.pages);
            let update = PickUpdate {
                stream: flight.stream,
                x: flight.x,
                y: flight.y,
                pick,
            };
            self.streams.push(update, flight.sequence);
        }
        Ok(())
    }

    fn dispatch_streams(&mut self) -> Result<(), RenderError> {
        let capacity = self.scene_gpu.pick_page_capacity();
        let mut encoder = None;
        while self.streams.has_room() {
            let Some(index) = self
                .streams
                .slots
                .iter()
                .position(|slot| slot.flight.is_none())
            else {
                break;
            };
            let Some((stream, x, y)) = self.streams.next_request() else {
                break;
            };
            self.streams.sequence += 1;
            let sequence = self.streams.sequence;
            let in_bounds = x < self.width && y < self.height;
            let slot = &mut self.streams.slots[index];
            let recorded = match (&self.pool, in_bounds) {
                (Some(pool), true) => {
                    let encoder =
                        encoder.get_or_insert_with(|| self.device.create_command_encoder());
                    encode_pick_copies::<D>(encoder, pool, (x, y), &slot.buffer)
                }
                _ => false,
            };
            if !recorded {
                let update = PickUpdate {
                    stream,
                    x,
                    y,
                    pick: Ok(None),
                };
                self.streams.push(update, sequence);
                continue;
            }
            if slot.pages.len() != capacity {
                slot.pages = vec![None; capacity].into_boxed_slice();
            }
            self.scene_gpu.capture_pick_submission(&mut slot.pages)?;
            slot.flight = Some(Flight {
                stream,
                x,
                y,
                sequence,
                mapped: false,
            });
        }
        if let Some(encoder) = encoder {
            self.queue.submit(encoder);
        }
        for slot in &mut self.streams.slots {
            if let Some(flight) = &mut slot.flight
                && !flight.mapped
            {
                if let Err(error) = slot.readback.begin_map(0, PICK_READBACK_BYTES) {
                    slot.flight = None;
                    return Err(error.into());
                }
                flight.mapped = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "pick_stream_tests.rs"]
mod tests;

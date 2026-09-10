//! Bounded, history-preserving off-screen sequence submission.

use super::image::{ImagePurpose, PendingImage};
use super::{Engine, Image, ImageConfig};
use crate::RenderError;
use pdviewx_core::Scene;
use pdviewx_gpu::{Device, Queue as _};
use pdviewx_math::Camera;
use std::collections::VecDeque;
use std::fmt;

/// Fixed output and readback limits for one deterministic frame sequence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SequenceConfig {
    /// Dimensions shared by every frame.
    pub image: ImageConfig,
    /// Nanoseconds represented by one timestamp tick.
    pub timebase_nanoseconds: u64,
    /// Bounded number of submitted frames awaiting readback.
    pub max_in_flight: u8,
}

impl SequenceConfig {
    /// Creates a nanosecond timebase for a fixed integer frame rate.
    ///
    /// # Errors
    ///
    /// Rejects zero frame rates and rates that do not map to a positive
    /// integral nanosecond interval.
    pub fn at_fps(
        image: ImageConfig,
        frames_per_second: u32,
        max_in_flight: u8,
    ) -> Result<Self, RenderError> {
        let timebase_nanoseconds = 1_000_000_000_u64
            .checked_div(u64::from(frames_per_second))
            .filter(|value| *value > 0)
            .ok_or(RenderError::InvalidSequence {
                reason: "frame rate must map to a positive nanosecond interval",
            })?;
        let config = Self {
            image,
            timebase_nanoseconds,
            max_in_flight,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(self) -> Result<(), RenderError> {
        if self.timebase_nanoseconds == 0 {
            return Err(RenderError::InvalidSequence {
                reason: "timebase must be positive",
            });
        }
        if !(2..=3).contains(&self.max_in_flight) {
            return Err(RenderError::InvalidSequence {
                reason: "max_in_flight must be two or three",
            });
        }
        Ok(())
    }
}

/// Stable identity and timestamp for one submitted sequence frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameTicket {
    /// Monotonic frame number within this sequence.
    pub index: u64,
    /// Caller timestamp in configured timebase ticks.
    pub timestamp: u64,
}

/// One completed, ordered sequence frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SequenceFrame {
    /// Submission identity retained through readback.
    pub ticket: FrameTicket,
    /// Tightly packed caller-owned pixels.
    pub image: Image,
}

struct PendingSequence<D: Device> {
    ticket: FrameTicket,
    image: PendingImage<D>,
}

/// A bounded sequence pipeline for one engine/backend type.
///
/// Submissions preserve temporal history and do not wait for readback. `poll`
/// resolves only a completed oldest frame, preserving presentation order.
pub struct SequenceRenderer<D: Device> {
    config: SequenceConfig,
    pending: VecDeque<PendingSequence<D>>,
    next_index: u64,
    last_timestamp: Option<u64>,
}

impl<D: Device> fmt::Debug for SequenceRenderer<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SequenceRenderer")
            .field("config", &self.config)
            .field("pending", &self.pending.len())
            .field("next_index", &self.next_index)
            .field("last_timestamp", &self.last_timestamp)
            .finish_non_exhaustive()
    }
}

impl<D: Device> Engine<D> {
    /// Opens a bounded history-preserving sequence pipeline.
    ///
    /// # Errors
    ///
    /// Rejects an invalid timebase or in-flight depth.
    pub fn sequence(&self, config: SequenceConfig) -> Result<SequenceRenderer<D>, RenderError> {
        config.validate()?;
        config
            .image
            .validate(self.device.capabilities().max_texture_dim)?;
        Ok(SequenceRenderer {
            config,
            pending: VecDeque::with_capacity(usize::from(config.max_in_flight)),
            next_index: 0,
            last_timestamp: None,
        })
    }
}

impl<D: Device> SequenceRenderer<D> {
    /// Submits one frame without waiting for GPU readback.
    ///
    /// # Errors
    ///
    /// Returns backpressure at the configured depth and rejects non-monotonic
    /// timestamps.
    pub fn submit(
        &mut self,
        engine: &mut Engine<D>,
        scene: &Scene,
        camera: &Camera,
        timestamp: u64,
    ) -> Result<FrameTicket, RenderError> {
        if self.pending.len() == usize::from(self.config.max_in_flight) {
            return Err(RenderError::SequenceBackpressure {
                max_in_flight: self.config.max_in_flight,
            });
        }
        if self
            .last_timestamp
            .is_some_and(|previous| timestamp <= previous)
        {
            return Err(RenderError::InvalidSequence {
                reason: "timestamps must increase strictly",
            });
        }
        let ticket = FrameTicket {
            index: self.next_index,
            timestamp,
        };
        self.next_index = self
            .next_index
            .checked_add(1)
            .ok_or(RenderError::InvalidSequence {
                reason: "frame index exhausted",
            })?;
        let image = engine.render_image_to_buffer(
            scene,
            camera,
            self.config.image,
            ImagePurpose::SequenceFrame,
        )?;
        self.pending.push_back(PendingSequence { ticket, image });
        self.last_timestamp = Some(timestamp);
        Ok(ticket)
    }

    /// Resolves the oldest frame only when its tracked submission has completed.
    ///
    /// # Errors
    ///
    /// Returns device loss or image-layout failures.
    pub fn poll(&mut self, engine: &mut Engine<D>) -> Result<Option<SequenceFrame>, RenderError> {
        let Some(front) = self.pending.front() else {
            return Ok(None);
        };
        let completed = engine.queue.completed_fence(&engine.device)?;
        if completed < front.image.completion() {
            return Ok(None);
        }
        self.resolve_front(engine).map(Some)
    }

    /// Drains every outstanding frame in submission order.
    ///
    /// # Errors
    ///
    /// Returns the first device or image-layout failure.
    pub fn finish(mut self, engine: &mut Engine<D>) -> Result<Vec<SequenceFrame>, RenderError> {
        let mut frames = Vec::with_capacity(self.pending.len());
        while !self.pending.is_empty() {
            frames.push(self.resolve_front(engine)?);
        }
        Ok(frames)
    }

    /// Number of submitted frames still owning readback buffers.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    fn resolve_front(&mut self, engine: &mut Engine<D>) -> Result<SequenceFrame, RenderError> {
        let Some(pending) = self.pending.pop_front() else {
            return Err(RenderError::InvalidSequence {
                reason: "no sequence frame is pending",
            });
        };
        let (buffer, size) = pending.image.readback();
        let mapped = engine
            .queue
            .read_buffer_blocking(&engine.device, buffer, 0, size)?;
        let image = pending.image.resolve(mapped, engine.target_format)?;
        Ok(SequenceFrame {
            ticket: pending.ticket,
            image,
        })
    }
}

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod tests;

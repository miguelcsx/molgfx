//! Bounded timestamp metadata collected at actual command-encoder pass boundaries.
//!
//! Storage and queries are allocated once. Move the capture into an encoder,
//! take it back before submission to resolve its written range, then retain it
//! until GPU completion and decoding before resetting for another submission.

use crate::{Device, GpuError};
use std::ops::Range;

/// Portable ceiling: each pass consumes two of the 4096 query slots.
pub const MAX_TIMESTAMP_CAPTURE_PASSES: u32 = 2048;

/// Failure to allocate an opt-in capture.
#[derive(Debug, thiserror::Error)]
pub enum TimestampCaptureError {
    /// The opened device does not support pass-boundary timestamp queries.
    #[error("pass timestamp queries are unsupported")]
    Unsupported,
    /// Capacity must be nonzero and fit the portable query-set ceiling.
    #[error("invalid timestamp capture capacity {requested}")]
    InvalidCapacity {
        /// Requested number of pass occurrences.
        requested: u32,
    },
    /// Query allocation failed.
    #[error(transparent)]
    Backend(#[from] GpuError),
}

/// The command kind at the actual recording boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimestampPassKind {
    /// A render pass.
    Render,
    /// A compute pass.
    Compute,
}

/// Two unique indices for one captured occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimestampQueryPair {
    /// Beginning-of-pass query index.
    pub beginning: u32,
    /// End-of-pass query index.
    pub end: u32,
}

/// Why one recorded pass has no capture-owned timestamp pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassTimestampAbsence {
    /// Caller-authored descriptor writes take precedence and remain unchanged.
    DescriptorTimestampWrites,
    /// Metadata-only capture has no supported timestamp query resource.
    Unsupported,
}

/// A pass occurrence, not a render-graph node or a label aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassTimestampRecord {
    /// Recording order, including occurrences omitted after capacity is exhausted.
    pub occurrence: u64,
    /// Descriptor label, borrowed without a string allocation.
    pub label: &'static str,
    /// Actual pass kind.
    pub kind: TimestampPassKind,
    /// Optional caller-provided temporal or exposure sample identity.
    pub sample: Option<u32>,
    /// Capture-owned pair or a reason that this descriptor could not be captured.
    pub queries: Result<TimestampQueryPair, PassTimestampAbsence>,
}

/// Why the bounded capture cannot represent every pass occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimestampCaptureIncomplete {
    /// Rendering continued unchanged, but the metadata/query budget was exhausted.
    CapacityExceeded {
        /// Configured maximum number of metadata records.
        capacity: u32,
        /// Number of omitted pass occurrences.
        omitted_passes: u64,
    },
}

/// Reusable storage and query ownership for one submission's actual passes.
#[derive(Debug)]
pub struct PassTimestampCapture<Q> {
    queries: Option<Q>,
    records: Vec<PassTimestampRecord>,
    max_passes: u32,
    next_query: u32,
    passes_seen: u64,
    sample: Option<u32>,
}

impl<Q> PassTimestampCapture<Q> {
    /// Allocates the query set and all metadata storage outside command recording.
    ///
    /// # Errors
    /// Returns typed unsupported, capacity, or backend allocation failures.
    pub fn new<D: Device<QuerySet = Q>>(
        device: &D,
        max_passes: u32,
    ) -> Result<Self, TimestampCaptureError> {
        if max_passes == 0 || max_passes > MAX_TIMESTAMP_CAPTURE_PASSES {
            return Err(TimestampCaptureError::InvalidCapacity {
                requested: max_passes,
            });
        }
        if !device.capabilities().timestamp_queries() {
            return Err(TimestampCaptureError::Unsupported);
        }
        let queries = device.create_timestamp_query_set(max_passes * 2)?;
        Ok(Self {
            queries: Some(queries),
            records: Vec::with_capacity(max_passes as usize),
            max_passes,
            next_query: 0,
            passes_seen: 0,
            sample: None,
        })
    }

    /// Allocates only bounded metadata for a device without timestamp queries.
    /// No query writes or synthetic timing values are produced.
    ///
    /// # Errors
    /// Returns a capacity error when the requested bound is invalid.
    pub fn metadata_only(max_passes: u32) -> Result<Self, TimestampCaptureError> {
        if max_passes == 0 || max_passes > MAX_TIMESTAMP_CAPTURE_PASSES {
            return Err(TimestampCaptureError::InvalidCapacity {
                requested: max_passes,
            });
        }
        Ok(Self {
            queries: None,
            records: Vec::with_capacity(max_passes as usize),
            max_passes,
            next_query: 0,
            passes_seen: 0,
            sample: None,
        })
    }

    /// Optional query resource; absent for metadata-only capture.
    #[must_use]
    pub const fn queries(&self) -> Option<&Q> {
        self.queries.as_ref()
    }

    /// Contiguous written query slots. Skip resolve when this range is empty.
    #[must_use]
    pub const fn query_range(&self) -> Range<u32> {
        0..self.next_query
    }

    /// Number of allocated query slots, including unused bounded capacity.
    #[must_use]
    pub const fn query_capacity(&self) -> u32 {
        if self.queries.is_some() {
            self.max_passes * 2
        } else {
            0
        }
    }

    /// Records retained in command recording order.
    #[must_use]
    pub fn records(&self) -> &[PassTimestampRecord] {
        &self.records
    }

    /// Total actual passes, including every omitted occurrence after overflow.
    #[must_use]
    pub const fn passes_seen(&self) -> u64 {
        self.passes_seen
    }

    /// Explicitly reports overflow; retained records must not imply completeness.
    #[must_use]
    pub fn incomplete(&self) -> Option<TimestampCaptureIncomplete> {
        let omitted_passes = self.passes_seen - self.records.len() as u64;
        (omitted_passes != 0).then_some(TimestampCaptureIncomplete::CapacityExceeded {
            capacity: self.max_passes,
            omitted_passes,
        })
    }

    /// Sets identity for subsequent pass occurrences; does not alter prior records.
    pub const fn set_sample(&mut self, sample: Option<u32>) {
        self.sample = sample;
    }

    /// Clears metadata without reallocating. Call only after the preceding GPU
    /// submission has completed and its queries have been decoded.
    pub fn reset(&mut self) {
        self.records.clear();
        self.next_query = 0;
        self.passes_seen = 0;
        self.sample = None;
    }

    /// Backend hook at pass creation. Descriptor-authored timestamps are never
    /// replaced, and no query is reused if the bounded capacity is exhausted.
    pub fn record_pass(
        &mut self,
        label: &'static str,
        kind: TimestampPassKind,
        descriptor_has_timestamps: bool,
    ) -> Option<TimestampQueryPair> {
        let occurrence = self.passes_seen;
        self.passes_seen = self.passes_seen.saturating_add(1);
        if self.records.len() == self.max_passes as usize {
            return None;
        }
        let queries = if descriptor_has_timestamps {
            Err(PassTimestampAbsence::DescriptorTimestampWrites)
        } else if self.queries.is_none() {
            Err(PassTimestampAbsence::Unsupported)
        } else {
            let pair = TimestampQueryPair {
                beginning: self.next_query,
                end: self.next_query + 1,
            };
            self.next_query += 2;
            Ok(pair)
        };
        self.records.push(PassTimestampRecord {
            occurrence,
            label,
            kind,
            sample: self.sample,
            queries,
        });
        queries.ok()
    }
}

#[cfg(test)]
#[path = "timestamp_capture_tests.rs"]
mod tests;

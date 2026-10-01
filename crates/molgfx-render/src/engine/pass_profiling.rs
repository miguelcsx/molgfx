//! Reusable capture storage owned until the exposure fence completes.

use super::GpuTiming;
use crate::RenderError;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder, Device, MAX_TIMESTAMP_CAPTURE_PASSES,
    PassTimestampAbsence, PassTimestampCapture, Queue, TimestampCaptureIncomplete,
    TimestampPassKind,
};

/// One actual render/compute pass occurrence, not a graph-node estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassTiming {
    /// Command recording order within this exposure.
    pub occurrence: u64,
    /// Static pass label; repeated labels remain distinct occurrences.
    pub label: &'static str,
    /// Actual encoder pass kind.
    pub kind: TimestampPassKind,
    /// Temporal sample associated with the commands.
    pub sample: Option<u32>,
    /// Device duration or an explicit absence reason.
    pub gpu_timing: GpuTiming,
    /// Raw device timestamp ticks for diagnosing unavailable durations.
    pub timestamp_ticks: Option<[u64; 2]>,
}

impl PassTiming {
    /// Stable command-kind label for telemetry consumers.
    #[must_use]
    pub const fn kind_name(self) -> &'static str {
        match self.kind {
            TimestampPassKind::Render => "render",
            TimestampPassKind::Compute => "compute",
        }
    }
}

/// Bounded capture coverage; overflow cannot masquerade as full timing.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct PassTimingCoverage {
    /// Actual passes recorded, including omitted occurrences.
    pub passes_seen: u64,
    /// Occurrences retained in bounded metadata storage.
    pub retained_passes: usize,
    /// Occurrences omitted after capacity exhaustion.
    pub omitted_passes: u64,
    /// Occurrences with resolved GPU duration, including valid zero durations.
    pub resolved_passes: usize,
}

#[derive(Debug)]
pub(super) struct PassProfiler<D: Device> {
    capture: Option<PassTimestampCapture<D::QuerySet>>,
    buffers: Option<TimestampBuffers<D>>,
    decoded: Vec<PassTiming>,
    coverage: PassTimingCoverage,
    scratch: Vec<u8>,
    previous_ticks: Vec<u64>,
    completed_tick_high: Option<u64>,
}

#[derive(Debug)]
struct TimestampBuffers<D: Device> {
    resolve: D::Buffer,
    readback: D::Buffer,
}

impl<D: Device> PassProfiler<D> {
    pub(super) fn new(device: &D) -> Result<Self, RenderError> {
        let capture = if device.capabilities().timestamp_queries() {
            PassTimestampCapture::new(device, MAX_TIMESTAMP_CAPTURE_PASSES)
        } else {
            PassTimestampCapture::metadata_only(MAX_TIMESTAMP_CAPTURE_PASSES)
        }
        .map_err(|error| match error {
            molgfx_gpu::TimestampCaptureError::Backend(error) => RenderError::Gpu(error),
            _ => RenderError::Residency {
                reason: "invalid pass profiling capacity",
            },
        })?;
        let buffers = if capture.queries().is_some() {
            let size = u64::from(capture.query_capacity()) * 8;
            Some(TimestampBuffers {
                resolve: device.create_buffer(&BufferDesc {
                    label: "pass timestamp resolve",
                    size,
                    usage: BufferUsage::QUERY_RESOLVE.union(BufferUsage::COPY_SRC),
                })?,
                readback: device.create_buffer(&BufferDesc {
                    label: "pass timestamp readback",
                    size,
                    usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
                })?,
            })
        } else {
            None
        };
        Ok(Self {
            capture: Some(capture),
            buffers,
            decoded: Vec::with_capacity(MAX_TIMESTAMP_CAPTURE_PASSES as usize),
            coverage: PassTimingCoverage::default(),
            scratch: vec![0; MAX_TIMESTAMP_CAPTURE_PASSES as usize * 2 * 8],
            previous_ticks: vec![0; MAX_TIMESTAMP_CAPTURE_PASSES as usize * 2],
            completed_tick_high: None,
        })
    }

    pub(super) fn attach(&mut self, encoder: &mut D::CommandEncoder) -> Result<(), RenderError> {
        let Some(mut capture) = self.capture.take() else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        capture.reset();
        self.decoded.clear();
        self.coverage = PassTimingCoverage::default();
        if encoder.set_timestamp_capture(Some(capture)).is_some() {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        }
        Ok(())
    }

    /// Recovers capture storage from commands that will never be submitted.
    pub(super) fn discard(&mut self, encoder: &mut D::CommandEncoder) -> Result<(), RenderError> {
        let Some(mut capture) = encoder.set_timestamp_capture(None) else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        capture.reset();
        self.capture = Some(capture);
        Ok(())
    }

    pub(super) fn detach(&mut self, encoder: &mut D::CommandEncoder) -> Result<(), RenderError> {
        let Some(capture) = encoder.set_timestamp_capture(None) else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        self.capture = Some(capture);
        Ok(())
    }

    /// Resolve only after sampling completion; early Metal resolves can read
    /// stale counter slots even when their command buffer finishes normally.
    pub(super) fn submit_resolve(
        &self,
        device: &D,
        queue: &D::Queue,
    ) -> Option<molgfx_gpu::FenceValue> {
        let (Some(capture), Some(buffers)) = (&self.capture, &self.buffers) else {
            return None;
        };
        let queries = capture.queries()?;
        let range = capture.query_range();
        if range.is_empty() {
            return None;
        }
        let size = u64::from(range.end) * 8;
        let mut encoder = device.create_command_encoder();
        encoder.resolve_query_set(queries, range, &buffers.resolve, 0);
        encoder.copy_buffer_to_buffer(&buffers.resolve, 0, &buffers.readback, 0, size);
        Some(queue.submit_tracked(encoder))
    }

    pub(super) async fn read_async(
        &mut self,
        device: &D,
        queue: &D::Queue,
    ) -> Result<GpuTiming, RenderError> {
        let size = self.read_size();
        if let Some(buffers) = &self.buffers
            && size != 0
        {
            queue
                .read_buffer_into_async(
                    device,
                    &buffers.readback,
                    0,
                    size as u64,
                    &mut self.scratch,
                )
                .await?;
        }
        self.decode_scratch(size, queue.timestamp_period())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn read(&mut self, device: &D, queue: &D::Queue) -> Result<GpuTiming, RenderError> {
        let size = self.read_size();
        if let Some(buffers) = &self.buffers
            && size != 0
        {
            queue.read_buffer_into_blocking(
                device,
                &buffers.readback,
                0,
                size as u64,
                &mut self.scratch,
            )?;
        }
        self.decode_scratch(size, queue.timestamp_period())
    }

    fn read_size(&self) -> usize {
        self.capture
            .as_ref()
            .map_or(0, |capture| capture.query_range().end as usize * 8)
    }

    fn decode_scratch(&mut self, size: usize, period: f32) -> Result<GpuTiming, RenderError> {
        Self::decode_storage(
            self.capture.as_ref(),
            &mut self.decoded,
            &mut self.coverage,
            &mut self.previous_ticks,
            &mut self.completed_tick_high,
            &self.scratch[..size],
            period,
        )
    }

    #[cfg(test)]
    fn decode(&mut self, data: &[u8], period: f32) -> Result<GpuTiming, RenderError> {
        Self::decode_storage(
            self.capture.as_ref(),
            &mut self.decoded,
            &mut self.coverage,
            &mut self.previous_ticks,
            &mut self.completed_tick_high,
            data,
            period,
        )
    }

    fn decode_storage(
        capture: Option<&PassTimestampCapture<D::QuerySet>>,
        decoded: &mut Vec<PassTiming>,
        coverage: &mut PassTimingCoverage,
        previous_ticks: &mut [u64],
        completed_tick_high: &mut Option<u64>,
        data: &[u8],
        period: f32,
    ) -> Result<GpuTiming, RenderError> {
        let Some(capture) = capture else {
            return Err(molgfx_gpu::GpuError::DeviceLost.into());
        };
        let mut bounds: Option<(u64, u64)> = None;
        let mut unavailable = None;
        for record in capture.records() {
            let (mut gpu_timing, ticks) = match record.queries {
                Ok(pair) => decode_pair(data, pair.beginning, pair.end, period)?,
                Err(PassTimestampAbsence::Unsupported) => (GpuTiming::Unsupported, None),
                Err(PassTimestampAbsence::DescriptorTimestampWrites) => {
                    (GpuTiming::DescriptorWrites, None)
                }
            };
            if let (Ok(pair), Some((start, end))) = (record.queries, ticks) {
                // Sequential completion fences exclude cross-output GPU work.
                // Require exact same-slot reuse as well as an older timestamp;
                // overlapping passes and coarse equal timestamps remain valid.
                let beginning = pair.beginning as usize;
                let ending = pair.end as usize;
                let reused = start == previous_ticks[beginning] || end == previous_ticks[ending];
                if gpu_timing.nanoseconds().is_some()
                    && reused
                    && completed_tick_high.is_some_and(|high| start < high)
                {
                    gpu_timing = GpuTiming::Stale;
                }
                previous_ticks[beginning] = start;
                previous_ticks[ending] = end;
            }
            if let Some((start, end)) = ticks
                && gpu_timing.nanoseconds().is_some()
            {
                bounds = Some(
                    bounds.map_or((start, end), |(low, high)| (low.min(start), high.max(end))),
                );
            } else if unavailable.is_none() {
                unavailable = Some(gpu_timing);
            }
            decoded.push(PassTiming {
                occurrence: record.occurrence,
                label: record.label,
                kind: record.kind,
                sample: record.sample,
                gpu_timing,
                timestamp_ticks: ticks.map(|(start, end)| [start, end]),
            });
        }
        if let Some((_, high)) = bounds {
            *completed_tick_high =
                Some(completed_tick_high.map_or(high, |previous| previous.max(high)));
        }
        let omitted_passes = match capture.incomplete() {
            Some(TimestampCaptureIncomplete::CapacityExceeded { omitted_passes, .. }) => {
                omitted_passes
            }
            None => 0,
        };
        *coverage = PassTimingCoverage {
            passes_seen: capture.passes_seen(),
            retained_passes: decoded.len(),
            omitted_passes,
            resolved_passes: decoded
                .iter()
                .filter(|pass| pass.gpu_timing.nanoseconds().is_some())
                .count(),
        };
        if omitted_passes != 0 {
            return Ok(GpuTiming::CapacityExceeded);
        }
        if let Some(reason) = unavailable {
            return Ok(reason);
        }
        match bounds {
            Some((start, end)) => GpuTiming::from_timestamps(start, end, period),
            None => Ok(GpuTiming::Unresolved),
        }
    }

    pub(super) fn timings(&self) -> &[PassTiming] {
        &self.decoded
    }
    pub(super) const fn coverage(&self) -> PassTimingCoverage {
        self.coverage
    }
}

fn decode_pair(
    data: &[u8],
    start: u32,
    end: u32,
    period: f32,
) -> Result<(GpuTiming, Option<(u64, u64)>), RenderError> {
    let (Some(start), Some(end)) = (read_tick(data, start), read_tick(data, end)) else {
        return Ok((GpuTiming::Malformed, None));
    };
    let timing = GpuTiming::from_timestamps(start, end, period)?;
    Ok((timing, Some((start, end))))
}

fn read_tick(data: &[u8], index: u32) -> Option<u64> {
    let offset = usize::try_from(index).ok()?.checked_mul(8)?;
    Some(u64::from_le_bytes(
        data.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}

#[cfg(test)]
#[path = "pass_profiling_tests.rs"]
mod tests;

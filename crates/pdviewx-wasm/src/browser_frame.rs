//! Reusable browser frame reports and explicit profiling snapshots.

use pdviewx::{FrameCompleteness, FrameReport, FrameStatus, FrameTiming};
use wasm_bindgen::prelude::*;

/// Allocation-stable summary filled by `WebEngine.renderInto`.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct WebFrameReport {
    presented: bool,
    complete: bool,
    pending_chunks: u64,
    needs_another_frame: bool,
    streaming_proxy: bool,
    adaptive_resolution: bool,
    tracked_chunks: u32,
    upload_in_flight_bytes: u64,
    derived_cache_gpu_bytes: u64,
    derived_cache_peak_gpu_bytes: u64,
    accounted_gpu_bytes: u64,
    physical_buffer_bytes: u64,
    physical_texture_bytes: u64,
    physical_peak_bytes: u64,
    render_width: u32,
    render_height: u32,
    render_scale: f32,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebFrameReport {
    /// Creates one report for reuse across the caller's frame loop.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
    /// Whether this frame reached the canvas surface.
    #[wasm_bindgen(getter)]
    pub fn presented(&self) -> bool {
        self.presented
    }
    /// Whether all requested resident detail contributed.
    #[wasm_bindgen(getter)]
    pub fn complete(&self) -> bool {
        self.complete
    }
    /// Number of provider chunks still pending.
    #[wasm_bindgen(getter, js_name = pendingChunks)]
    pub fn pending_chunks(&self) -> u64 {
        self.pending_chunks
    }
    /// Whether the caller should schedule another animation frame.
    #[wasm_bindgen(getter, js_name = needsAnotherFrame)]
    pub fn needs_another_frame(&self) -> bool {
        self.needs_another_frame
    }
    /// Whether a non-resident streaming proxy was visible.
    #[wasm_bindgen(getter, js_name = streamingProxy)]
    pub fn streaming_proxy(&self) -> bool {
        self.streaming_proxy
    }
    /// Whether the requested native dimensions were uniformly reduced.
    #[wasm_bindgen(getter, js_name = adaptiveResolution)]
    pub fn adaptive_resolution(&self) -> bool {
        self.adaptive_resolution
    }
    /// Whether any explicit degradation was used.
    #[wasm_bindgen(getter)]
    pub fn degraded(&self) -> bool {
        self.streaming_proxy || self.adaptive_resolution
    }
    /// Number of chunks tracked by engine residency.
    #[wasm_bindgen(getter, js_name = trackedChunks)]
    pub fn tracked_chunks(&self) -> u32 {
        self.tracked_chunks
    }
    /// Upload bytes protected by unfinished GPU work.
    #[wasm_bindgen(getter, js_name = uploadInFlightBytes)]
    pub fn upload_in_flight_bytes(&self) -> u64 {
        self.upload_in_flight_bytes
    }
    /// Retained recomputable GPU bytes.
    #[wasm_bindgen(getter, js_name = derivedCacheGpuBytes)]
    pub fn derived_cache_gpu_bytes(&self) -> u64 {
        self.derived_cache_gpu_bytes
    }
    /// Peak retained recomputable GPU bytes since engine construction.
    #[wasm_bindgen(getter, js_name = derivedCachePeakGpuBytes)]
    pub fn derived_cache_peak_gpu_bytes(&self) -> u64 {
        self.derived_cache_peak_gpu_bytes
    }
    /// GPU bytes covered by the currently available core counters.
    #[wasm_bindgen(getter, js_name = accountedGpuBytes)]
    pub fn accounted_gpu_bytes(&self) -> u64 {
        self.accounted_gpu_bytes
    }
    /// Live bytes requested for engine-owned WebGPU buffers.
    #[wasm_bindgen(getter, js_name = physicalBufferBytes)]
    pub fn physical_buffer_bytes(&self) -> u64 {
        self.physical_buffer_bytes
    }
    /// Live bytes requested for engine-owned WebGPU textures.
    #[wasm_bindgen(getter, js_name = physicalTextureBytes)]
    pub fn physical_texture_bytes(&self) -> u64 {
        self.physical_texture_bytes
    }
    /// Peak live buffer and texture bytes since device creation.
    #[wasm_bindgen(getter, js_name = physicalPeakBytes)]
    pub fn physical_peak_bytes(&self) -> u64 {
        self.physical_peak_bytes
    }
    /// Physical width actually rendered.
    #[wasm_bindgen(getter, js_name = renderWidth)]
    pub fn render_width(&self) -> u32 {
        self.render_width
    }
    /// Physical height actually rendered.
    #[wasm_bindgen(getter, js_name = renderHeight)]
    pub fn render_height(&self) -> u32 {
        self.render_height
    }
    /// Uniform ratio between requested and rendered physical pixels.
    #[wasm_bindgen(getter, js_name = renderScale)]
    pub fn render_scale(&self) -> f32 {
        self.render_scale
    }
}

impl WebFrameReport {
    pub(crate) fn write(&mut self, report: FrameReport, extent: WebRenderExtent) {
        self.pending_chunks = match report.completeness {
            FrameCompleteness::Complete => 0,
            FrameCompleteness::Progressive { pending_chunks } => pending_chunks,
        };
        self.presented = report.status == FrameStatus::Presented;
        self.complete = self.pending_chunks == 0;
        self.needs_another_frame = report.needs_another_frame;
        self.streaming_proxy = report
            .degradation
            .contains(pdviewx::FrameDegradation::STREAMING_PROXY);
        self.adaptive_resolution = extent.scale < 1.0;
        self.tracked_chunks = match u32::try_from(report.metrics.tracked_chunks) {
            Ok(value) => value,
            Err(_) => u32::MAX,
        };
        self.upload_in_flight_bytes = report.metrics.upload_in_flight_bytes;
        self.derived_cache_gpu_bytes = report.metrics.derived_cache_gpu_bytes;
        self.derived_cache_peak_gpu_bytes = report.metrics.derived_cache_peak_gpu_bytes;
        self.accounted_gpu_bytes = report.metrics.physical_total_bytes;
        self.physical_buffer_bytes = report.metrics.physical_buffer_bytes;
        self.physical_texture_bytes = report.metrics.physical_texture_bytes;
        self.physical_peak_bytes = report.metrics.physical_peak_bytes;
        self.render_width = extent.width;
        self.render_height = extent.height;
        self.render_scale = extent.scale;
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WebRenderExtent {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale: f32,
}

/// Explicit asynchronous timing sample, separate from the hot presentation path.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebFrameTiming {
    inner: FrameTiming,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebFrameTiming {
    /// Resolved GPU duration in nanoseconds, or zero when unavailable.
    #[wasm_bindgen(getter, js_name = gpuNanoseconds)]
    pub fn gpu_nanoseconds(&self) -> u64 {
        self.inner.gpu_ns
    }
    /// Whether the adapter returned a real GPU timestamp duration.
    #[wasm_bindgen(getter, js_name = gpuTimingResolved)]
    pub fn gpu_timing_resolved(&self) -> bool {
        self.inner.gpu_timing_resolved
    }
    /// CPU command construction and submission duration in nanoseconds.
    #[wasm_bindgen(getter, js_name = cpuNanoseconds)]
    pub fn cpu_nanoseconds(&self) -> u64 {
        self.inner.cpu_ns
    }
    /// End-to-end measured frame duration in nanoseconds.
    #[wasm_bindgen(getter, js_name = frameNanoseconds)]
    pub fn frame_nanoseconds(&self) -> u64 {
        self.inner.frame_ns
    }
    /// Residency allocation events observed by the profiled frame.
    #[wasm_bindgen(getter, js_name = allocationEvents)]
    pub fn allocation_events(&self) -> u64 {
        self.inner.residency_counters().allocation_events
    }
    /// Bytes uploaded while producing the profiled frame.
    #[wasm_bindgen(getter, js_name = uploadBytes)]
    pub fn upload_bytes(&self) -> u64 {
        self.inner.residency_counters().upload_bytes
    }
    /// Resident bytes reported by the residency subsystem.
    #[wasm_bindgen(getter, js_name = residentBytes)]
    pub fn resident_bytes(&self) -> u64 {
        self.inner.residency_counters().resident_bytes
    }
    /// Capacity or synchronization stalls observed by the profiled frame.
    #[wasm_bindgen(getter, js_name = stallEvents)]
    pub fn stall_events(&self) -> u64 {
        self.inner.residency_counters().stall_events
    }
}

impl From<FrameTiming> for WebFrameTiming {
    fn from(inner: FrameTiming) -> Self {
        Self { inner }
    }
}

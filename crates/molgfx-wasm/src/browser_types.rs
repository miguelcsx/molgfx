//! Fixed-size JavaScript values for browser configuration and frame telemetry.

use molgfx::{
    Capabilities, EntityKind, Pick, PickEntity, PowerPreference, RenderMode, RenderProfile,
};
use wasm_bindgen::prelude::*;

const DEFAULT_MEMORY_BYTES: u64 = 512 * 1024 * 1024;

/// Adapter preference when multiple WebGPU adapters are available.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebPowerPreference {
    /// Prefer the fastest available adapter.
    HighPerformance,
    /// Prefer the most energy-efficient available adapter.
    LowPower,
}

/// Renderer-owned presentation profiles available without JS-side graph logic.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebProfile {
    /// Neutral quantitative inspection.
    Inspection,
    /// Restrained publication illustration.
    Illustrative,
    /// Film-style optics and grading.
    Cinematic,
}

/// Interactive or progressive rendering strategy.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebRenderMode {
    /// Interactive raster rendering.
    Realtime,
    /// Progressive high-fidelity rendering.
    Cinematic,
}

/// Policy for reconciling requested pixels with the configured memory ceiling.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebResolutionPolicy {
    /// Preserve every requested physical pixel or return a typed budget error.
    NativeStrict,
    /// Reduce both axes uniformly only when the requested surface exceeds its budget.
    Adaptive,
}

/// Explicit browser GPU lifecycle state.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebLifecycleState {
    /// The device and presentation surface are ready.
    Ready,
    /// A skipped frame reconfigured the surface; submit another frame.
    SurfaceRecovering,
    /// The device was lost and must be recreated explicitly.
    DeviceLost,
    /// GPU resources were released and this engine cannot be recovered.
    Destroyed,
}

/// Validated browser engine configuration.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct WebEngineConfig {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) target_fps: u16,
    pub(crate) total_memory_bytes: u64,
    pub(crate) power: WebPowerPreference,
    pub(crate) profile: WebProfile,
    pub(crate) mode: WebRenderMode,
    pub(crate) resolution_policy: WebResolutionPolicy,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebEngineConfig {
    /// Creates the browser-first default: 120 Hz, 512 MiB, inspection profile.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            target_fps: 120,
            total_memory_bytes: DEFAULT_MEMORY_BYTES,
            power: WebPowerPreference::HighPerformance,
            profile: WebProfile::Inspection,
            mode: WebRenderMode::Realtime,
            resolution_policy: WebResolutionPolicy::NativeStrict,
        }
    }

    /// Changes the requested frame-rate target used by the caller scheduler.
    #[wasm_bindgen(js_name = setTargetFps)]
    pub fn set_target_fps(&mut self, value: u16) {
        self.target_fps = value.clamp(1, 1000);
    }
    /// Changes the total engine budget in bytes.
    #[wasm_bindgen(js_name = setTotalMemoryBytes)]
    pub fn set_total_memory_bytes(&mut self, value: u64) {
        self.total_memory_bytes = value.max(64 * 1024 * 1024);
    }
    /// Selects the WebGPU adapter preference.
    #[wasm_bindgen(js_name = setPowerPreference)]
    pub fn set_power_preference(&mut self, value: WebPowerPreference) {
        self.power = value;
    }
    /// Selects the initial renderer profile.
    #[wasm_bindgen(js_name = setProfile)]
    pub fn set_profile(&mut self, value: WebProfile) {
        self.profile = value;
    }
    /// Selects the initial render mode.
    #[wasm_bindgen(js_name = setMode)]
    pub fn set_mode(&mut self, value: WebRenderMode) {
        self.mode = value;
    }
    /// Selects strict native pixels or explicit uniform adaptive scaling.
    #[wasm_bindgen(js_name = setResolutionPolicy)]
    pub fn set_resolution_policy(&mut self, value: WebResolutionPolicy) {
        self.resolution_policy = value;
    }
    /// Configured total budget.
    #[wasm_bindgen(getter, js_name = totalMemoryBytes)]
    pub fn total_memory_bytes(&self) -> u64 {
        self.total_memory_bytes
    }
    /// Requested scheduler rate.
    #[wasm_bindgen(getter, js_name = targetFps)]
    pub fn target_fps(&self) -> u16 {
        self.target_fps
    }
    /// Configured resolution policy.
    #[wasm_bindgen(getter, js_name = resolutionPolicy)]
    pub fn resolution_policy(&self) -> WebResolutionPolicy {
        self.resolution_policy
    }
}

impl WebEngineConfig {
    pub(crate) fn power(&self) -> PowerPreference {
        match self.power {
            WebPowerPreference::HighPerformance => PowerPreference::HighPerformance,
            WebPowerPreference::LowPower => PowerPreference::LowPower,
        }
    }
    pub(crate) fn profile(&self) -> RenderProfile {
        match self.profile {
            WebProfile::Inspection => RenderProfile::inspection(),
            WebProfile::Illustrative => RenderProfile::illustrative(),
            WebProfile::Cinematic => RenderProfile::cinematic(),
        }
    }
    pub(crate) fn mode(&self) -> RenderMode {
        match self.mode {
            WebRenderMode::Realtime => RenderMode::Realtime,
            WebRenderMode::Cinematic => RenderMode::Cinematic,
        }
    }
}

/// Device limits and optional accelerated paths detected at creation.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebCapabilities {
    inner: Capabilities,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebCapabilities {
    /// Largest single storage-buffer binding.
    #[wasm_bindgen(getter, js_name = maxStorageBufferBytes)]
    pub fn max_storage_buffer_bytes(&self) -> u64 {
        self.inner.max_storage_buffer_bytes
    }
    /// Storage-buffer bindings visible to one shader stage.
    #[wasm_bindgen(getter, js_name = maxStorageBuffersPerShaderStage)]
    pub fn max_storage_buffers(&self) -> u32 {
        self.inner.max_storage_buffers_per_shader_stage
    }
    /// Largest two-dimensional texture extent.
    #[wasm_bindgen(getter, js_name = maxTextureDimension2D)]
    pub fn max_texture_2d(&self) -> u32 {
        self.inner.max_texture_dim
    }
    /// Largest three-dimensional texture extent.
    #[wasm_bindgen(getter, js_name = maxTextureDimension3D)]
    pub fn max_texture_3d(&self) -> u32 {
        self.inner.max_texture_dim_3d
    }
    /// Whether optional subgroup operations are accelerated.
    #[wasm_bindgen(getter)]
    pub fn subgroups(&self) -> bool {
        self.inner.subgroup_ops()
    }
    /// Whether GPU timestamp queries are available.
    #[wasm_bindgen(getter)]
    pub fn timestamps(&self) -> bool {
        self.inner.timestamp_queries()
    }
    /// Whether hardware ray queries are available.
    #[wasm_bindgen(getter, js_name = rayQueries)]
    pub fn ray_queries(&self) -> bool {
        self.inner.ray_query()
    }
}

impl From<Capabilities> for WebCapabilities {
    fn from(inner: Capabilities) -> Self {
        Self { inner }
    }
}

/// Stable kind of a picked visible entity.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebPickKind {
    /// Molecular atom.
    Atom,
    /// Molecular bond.
    Bond,
    /// Interaction edge.
    Edge,
    /// Text label.
    Label,
    /// Caller-authored analytic primitive.
    Primitive,
    /// Caller-authored indexed mesh.
    Mesh,
    /// Compact ligand pose batch.
    LigandPoseBatch,
    /// Analytic guide.
    Guide,
    /// Dynamic trajectory bond.
    DynamicBond,
    /// Generic point row.
    Point,
    /// Generic rigid instance.
    Instance,
    /// Analytic template part.
    TemplatePart,
    /// Generic relation row.
    Relation,
    /// Categorical volume label.
    VolumeSegment,
}

/// Compact pick result; large identities cross as JavaScript `BigInt` values.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebPick {
    kind: WebPickKind,
    dataset: u64,
    chunk: u64,
    row: u64,
    volume: u32,
    label: u32,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebPick {
    /// Picked entity namespace.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> WebPickKind {
        self.kind
    }
    /// Stable dataset identity, or zero for a volume segment.
    #[wasm_bindgen(getter)]
    pub fn dataset(&self) -> u64 {
        self.dataset
    }
    /// Stable chunk identity, or zero for a volume segment.
    #[wasm_bindgen(getter)]
    pub fn chunk(&self) -> u64 {
        self.chunk
    }
    /// Stable logical row, or zero for a volume segment.
    #[wasm_bindgen(getter)]
    pub fn row(&self) -> u64 {
        self.row
    }
    /// Segmentation slot, or `u32::MAX` for structural entities.
    #[wasm_bindgen(getter)]
    pub fn volume(&self) -> u32 {
        self.volume
    }
    /// Exact segmentation label, or `u32::MAX` for structural entities.
    #[wasm_bindgen(getter)]
    pub fn label(&self) -> u32 {
        self.label
    }
}

impl From<Pick> for WebPick {
    fn from(pick: Pick) -> Self {
        match pick.entity {
            PickEntity::Structure(identity) => Self {
                kind: entity_kind(identity.kind()),
                dataset: identity.dataset().get(),
                chunk: identity.chunk().get(),
                row: identity.row().get(),
                volume: u32::MAX,
                label: u32::MAX,
            },
            PickEntity::VolumeSegment(segment) => Self {
                kind: WebPickKind::VolumeSegment,
                dataset: 0,
                chunk: 0,
                row: 0,
                volume: segment.volume.row(),
                label: segment.label,
            },
        }
    }
}

fn entity_kind(kind: EntityKind) -> WebPickKind {
    match kind {
        EntityKind::Atom => WebPickKind::Atom,
        EntityKind::Bond => WebPickKind::Bond,
        EntityKind::Edge => WebPickKind::Edge,
        EntityKind::Label => WebPickKind::Label,
        EntityKind::Primitive => WebPickKind::Primitive,
        EntityKind::Mesh => WebPickKind::Mesh,
        EntityKind::LigandPoseBatch => WebPickKind::LigandPoseBatch,
        EntityKind::Guide => WebPickKind::Guide,
        EntityKind::DynamicBond => WebPickKind::DynamicBond,
        EntityKind::Point => WebPickKind::Point,
        EntityKind::Instance => WebPickKind::Instance,
        EntityKind::TemplatePart => WebPickKind::TemplatePart,
        EntityKind::Relation => WebPickKind::Relation,
    }
}

//! Load-time state for every render pass.

use super::{
    AmbientOcclusionPass, AoDenoisePass, BloomPass, BondPass, CartoonPass, CullPass,
    DepthOfFieldPass, InteractionPass, LabelPass, LightingPass, MotionBlurPass, OitCompositePass,
    OitPass, OverlayPass, ParticleMotionPass, PointPass, PrimitivePass, ShadowPass, SpherePass,
    SurfaceFieldPass, SurfacePass, TemporalPass, TonemapPass, TrajectoryPass,
};
use pdviewx_gpu::Device;

/// Every pass's load-time state, owned by the engine and exposed read-only
/// to record functions through the context. Fields grow as passes land.
#[derive(Debug)]
pub struct PassRegistry<D: Device> {
    /// The sphere impostor pass.
    pub(crate) sphere: SpherePass<D>,
    /// Pixel-stable atom points.
    pub(crate) point: PointPass<D>,
    /// Analytic caller-authored primitives.
    pub(crate) primitive: PrimitivePass<D>,
    /// BVH-bounded implicit molecular surfaces.
    pub(crate) surface: SurfacePass<D>,
    /// Persistent solvent-excluded field generation.
    pub(crate) surface_field: SurfaceFieldPass<D>,
    /// Analytic bond capsules.
    pub(crate) bond: BondPass<D>,
    /// Transport-framed polymer cartoons.
    pub(crate) cartoon: CartoonPass<D>,
    /// GPU visibility compaction and indirect arguments.
    pub(crate) cull: CullPass<D>,
    /// Tile-classified physical camera depth of field.
    pub(crate) depth_of_field: DepthOfFieldPass<D>,
    /// Deterministic cavity/contact ambient occlusion.
    pub(crate) ambient_occlusion: AmbientOcclusionPass<D>,
    /// Edge-aware denoise of the traced occlusion buffer.
    pub(crate) ao_denoise: AoDenoisePass<D>,
    /// Shared HDR deferred molecular lighting.
    pub(crate) lighting: LightingPass<D>,
    /// Scene-fit analytic atom and bond shadows.
    pub(crate) shadow: ShadowPass<D>,
    /// Reprojected temporal anti-aliasing.
    pub(crate) temporal: TemporalPass<D>,
    /// Weighted-blended translucent geometry.
    pub(crate) oit: OitPass<D>,
    /// Caller-supplied analytic interaction glyphs.
    pub(crate) interaction: InteractionPass<D>,
    /// Persistent annotations, markers and typed measurements.
    pub(crate) label: LabelPass<D>,
    /// Composites translucent accumulation over opaque HDR.
    pub(crate) oit_composite: OitCompositePass<D>,
    /// Bright-pass highlight bleed.
    pub(crate) bloom: BloomPass<D>,
    /// Camera-shutter blur driven by the opaque motion-vector buffer.
    pub(crate) motion_blur: MotionBlurPass<D>,
    /// HDR presentation transform.
    pub(crate) tonemap: TonemapPass<D>,
    /// Depth-independent post-tonemap presentation overlays.
    pub(crate) overlay: OverlayPass<D>,
    /// Topology-stable coordinate interpolation.
    pub(crate) trajectory: TrajectoryPass<D>,
    /// Fixed-step caller-supplied visual particle advection.
    pub(crate) particle_motion: ParticleMotionPass<D>,
}

//! Engine construction: device open, pass creation, graph declaration.

use super::graph_setup::{realtime_nodes, realtime_resources};
use super::{
    EngineConfig, FocusTracker, GpuProfiler, PassRegistry, Picker, RenderMode, RenderProfile,
    ResolvedRenderPlan, TemporalState,
};
use crate::error::RenderError;
use crate::graph::{self, PassNode, ResourceDesc, TransientPool};
use crate::passes::{
    AmbientOcclusionPass, AoDenoisePass, BloomPass, BondPass, CartoonPass, CullPass,
    DepthOfFieldPass, FrameBindings, InteractionPass, LabelPass, LightingPass, MotionBlurPass,
    OitCompositePass, OitPass, OverlayPass, ParticleMotionPass, PointPass, PrimitivePass,
    ShadowPass, SpherePass, SurfaceFieldPass, SurfacePass, TemporalPass, TonemapPass,
    TrajectoryPass,
};
use crate::scene_gpu::GpuScene;
use pdviewx_gpu::{Device, DeviceDesc, Opened, TextureFormat, WindowTarget};

/// The rendering engine, generic over the device with no dynamic dispatch
/// on the frame path.
#[derive(Debug)]
pub struct Engine<D: Device> {
    pub(crate) device: D,
    pub(crate) queue: D::Queue,
    pub(crate) surface: Option<D::Surface>,
    pub(crate) passes: PassRegistry<D>,
    pub(crate) resources: Vec<ResourceDesc>,
    pub(crate) pass_nodes: Vec<PassNode<D>>,
    pub(crate) order: Vec<usize>,
    pub(crate) pool: Option<TransientPool<D>>,
    pub(crate) bindings: Option<FrameBindings<D>>,
    pub(crate) scene_gpu: GpuScene<D>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) target_format: TextureFormat,
    pub(crate) profiler: Option<GpuProfiler<D>>,
    pub(crate) picker: Picker<D>,
    pub(crate) temporal: TemporalState,
    pub(crate) mode: RenderMode,
    pub(crate) profile: RenderProfile,
    pub(crate) resolved_plan: ResolvedRenderPlan,
    pub(crate) focus_tracker: FocusTracker,
}

fn realtime_passes<D: Device>(
    device: &D,
    target_format: TextureFormat,
    scene: &GpuScene<D>,
) -> Result<PassRegistry<D>, RenderError> {
    Ok(PassRegistry {
        sphere: SpherePass::new(
            device,
            target_format,
            &scene.group0_layout,
            &scene.group2_layout,
        )?,
        point: PointPass::new(device, &scene.group0_layout, &scene.group2_layout)?,
        primitive: PrimitivePass::new(device, &scene.group0_layout, &scene.primitive_layout)?,
        surface: SurfacePass::new(device, &scene.group0_layout, &scene.group2_layout)?,
        surface_field: SurfaceFieldPass::new(
            device,
            &scene.surface_field_output_layout,
            &scene.surface_field_erosion_layout,
            &scene.surface_field_input_layout,
        )?,
        bond: BondPass::new(
            device,
            target_format,
            &scene.group0_layout,
            &scene.group2_layout,
        )?,
        cartoon: CartoonPass::new(device, &scene.group0_layout, &scene.ribbon_layout)?,
        cull: CullPass::new(device, &scene.cull_layout)?,
        depth_of_field: DepthOfFieldPass::new(device, &scene.group0_layout)?,
        ambient_occlusion: AmbientOcclusionPass::new(
            device,
            &scene.group0_layout,
            &scene.group2_layout,
        )?,
        ao_denoise: AoDenoisePass::new(device, &scene.group0_layout)?,
        lighting: LightingPass::new(device, &scene.group0_layout)?,
        shadow: ShadowPass::new(
            device,
            &scene.group0_layout,
            &scene.group2_layout,
            &scene.ribbon_layout,
            &scene.primitive_layout,
        )?,
        oit: OitPass::new(
            device,
            &scene.group0_layout,
            &scene.group2_layout,
            &scene.ribbon_layout,
            &scene.primitive_layout,
            &scene.volume_layout,
            &scene.segmentation_layout,
        )?,
        interaction: InteractionPass::new(device, &scene.group0_layout, &scene.interaction_layout)?,
        label: LabelPass::new(
            device,
            &scene.group0_layout,
            &scene.label_declutter_layout,
            &scene.label_render_layout,
        )?,
        oit_composite: OitCompositePass::new(device)?,
        temporal: TemporalPass::new(device, &scene.group0_layout)?,
        bloom: BloomPass::new(device, &scene.group0_layout)?,
        motion_blur: MotionBlurPass::new(device, &scene.group0_layout)?,
        tonemap: TonemapPass::new(device, target_format, &scene.group0_layout)?,
        overlay: OverlayPass::new(
            device,
            target_format,
            &scene.group0_layout,
            &scene.overlay_layout,
        )?,
        trajectory: TrajectoryPass::new(device, &scene.trajectory_layout)?,
        particle_motion: ParticleMotionPass::new(device, &scene.primitive_motion_layout)?,
    })
}

impl<D: Device> Engine<D> {
    /// Asynchronously opens a device and builds the realtime graph. Pass a
    /// window to render to screen; none for off-screen rendering.
    ///
    /// # Errors
    ///
    /// No compatible adapter, or pipeline construction failed.
    pub async fn new_async(
        config: &EngineConfig,
        window: Option<WindowTarget>,
    ) -> Result<Self, RenderError> {
        let opened = D::open_async(
            &DeviceDesc {
                power: config.power,
            },
            window,
        )
        .await?;
        Self::from_opened(config, opened)
    }

    /// Opens a native device synchronously for callers without an async
    /// executor. Browser callers use [`Self::new_async`].
    ///
    /// # Errors
    ///
    /// No compatible adapter, or pipeline construction failed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(config: &EngineConfig, window: Option<WindowTarget>) -> Result<Self, RenderError> {
        let opened = D::open_blocking(
            &DeviceDesc {
                power: config.power,
            },
            window,
        )?;
        Self::from_opened(config, opened)
    }

    fn from_opened(config: &EngineConfig, opened: Opened<D>) -> Result<Self, RenderError> {
        let mut surface = opened.surface;
        let device = opened.device;

        let target_format = super::target::configure(&device, &mut surface, config);

        let profile = config.profile.clone();
        let resolved_plan = profile.resolve();
        let scene_gpu = GpuScene::new(&device)?;
        let passes = realtime_passes(&device, target_format, &scene_gpu)?;
        let pass_nodes = realtime_nodes(
            resolved_plan.depth_of_field().is_some(),
            resolved_plan.bloom().is_some(),
            resolved_plan.motion_blur().is_some(),
        );
        let resources = realtime_resources();
        let order = graph::schedule(&pass_nodes)?;

        let profiler = GpuProfiler::new(&device)?;
        let picker = Picker::new(&device)?;
        Ok(Self {
            device,
            queue: opened.queue,
            surface,
            passes,
            resources,
            pass_nodes,
            order,
            pool: None,
            bindings: None,
            scene_gpu,
            width: config.width,
            height: config.height,
            target_format,
            profiler,
            picker,
            temporal: TemporalState::default(),
            mode: config.mode,
            profile,
            resolved_plan,
            focus_tracker: FocusTracker::default(),
        })
    }
}

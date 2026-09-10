//! Engine construction: device open, pass creation, graph declaration.

use super::graph_setup::{realtime_nodes, realtime_resources};
use super::{
    DerivedCache, EngineConfig, FocusTracker, GpuProfiler, PassRegistry, Picker, RenderMode,
    RenderProfile, ResolvedRenderPlan, ShadowBoundCache, TemporalState,
    chunk_residency::ChunkGpuResidency,
};
use crate::error::RenderError;
use crate::graph::{self, PassNode, ResourceDesc, TransientPool};
use crate::passes::{
    AmbientOcclusionPass, AoDenoisePass, BloomPass, BondPass, CartoonPass, CullPass,
    DepthOfFieldPass, FrameBindings, InteractionPass, LabelPass, LightingPass, MotionBlurPass,
    OccupancyPass, OitCompositePass, OitPass, OverlayPass, ParticleMotionPass, PointPass,
    PrimitivePass, RelationResolvePass, ShadowPass, SpherePass, SurfaceComponentPass,
    SurfaceFieldPass, SurfacePass, TemporalPass, TonemapPass, TrajectoryPass,
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
    pub(crate) temporal_scene_identity: Option<u64>,
    pub(crate) mode: RenderMode,
    pub(crate) profile: RenderProfile,
    pub(crate) resolved_plan: ResolvedRenderPlan,
    pub(crate) focus_tracker: FocusTracker,
    pub(crate) shadow_bound: ShadowBoundCache,
    pub(super) derived_cache: DerivedCache,
    pub(super) derived_frame: u64,
    pub(crate) host_working_set: pdviewx_core::HostWorkingSet,
    pub(super) chunk_residency: ChunkGpuResidency<D>,
}

fn cull_pass<D: Device>(device: &D, scene: &GpuScene<D>) -> Result<CullPass<D>, RenderError> {
    let layouts = crate::passes::CullLayouts {
        atoms: &scene.atom_cull_layout,
        bonds: &scene.bond_cull_layout,
        visuals: &scene.visual_cull_layout,
        frame: &scene.group0_layout,
        paged: scene.paged_chunk_layout(),
        paged_bonds: scene.paged_bond_layout(),
        points: &scene.generic_point_cull_layout,
        instances: &scene.generic_instance_cull_layout,
        instance_timeline: &scene.instance_timeline_layout,
        attribute_timeline: &scene.attribute_timeline_layout,
        relations: &scene.relation_cull_layout,
    };
    CullPass::new(device, &layouts)
}

fn realtime_passes<D: Device>(
    device: &D,
    target_format: TextureFormat,
    scene: &GpuScene<D>,
    plan: &ResolvedRenderPlan,
) -> Result<PassRegistry<D>, RenderError> {
    Ok(PassRegistry {
        sphere: SpherePass::new(
            device,
            target_format,
            &scene.group0_layout,
            &scene.group2_layout,
            scene.paged_chunk_layout(),
        )?,
        point: PointPass::new(
            device,
            &scene.group0_layout,
            &scene.group2_layout,
            scene.paged_chunk_layout(),
            &scene.generic_point_render_layout,
        )?,
        primitive: PrimitivePass::new(
            device,
            &scene.group0_layout,
            &scene.primitive_layout,
            &scene.ligand_pose_layout,
            &scene.generic_instance_render_layout,
        )?,
        surface: SurfacePass::new(device, &scene.group0_layout, &scene.group2_layout)?,
        surface_field: SurfaceFieldPass::new(
            device,
            &scene.surface_field_output_layout,
            &scene.surface_field_erosion_layout,
            &scene.surface_field_normal_layout,
            &scene.surface_field_input_layout,
        )?,
        surface_components: SurfaceComponentPass::new(device, &scene.surface_component_layout)?,
        bond: BondPass::new(
            device,
            target_format,
            &scene.group0_layout,
            &scene.group2_layout,
            scene.paged_bond_layout(),
        )?,
        cartoon: CartoonPass::new(device, &scene.group0_layout, &scene.ribbon_layout)?,
        cull: cull_pass(device, scene)?,
        depth_of_field: plan
            .depth_of_field()
            .map(|_| DepthOfFieldPass::new(device, &scene.group0_layout))
            .transpose()?,
        ambient_occlusion: AmbientOcclusionPass::new(
            device,
            &scene.group0_layout,
            &scene.quality_layout,
        )?,
        ao_denoise: AoDenoisePass::new(device, &scene.group0_layout)?,
        lighting: LightingPass::new(device, &scene.group0_layout)?,
        shadow: ShadowPass::new(
            device,
            &scene.group0_layout,
            &scene.group2_layout,
            &scene.ribbon_layout,
            &scene.primitive_shadow_layout,
            &scene.ligand_pose_layout,
        )?,
        oit: OitPass::new(
            device,
            &scene.group0_layout,
            &scene.group2_layout,
            &scene.ribbon_layout,
            (&scene.primitive_layout, &scene.ligand_pose_layout),
            (
                &scene.generic_point_render_layout,
                &scene.generic_instance_render_layout,
            ),
            (&scene.volume_layout, &scene.segmentation_layout),
        )?,
        interaction: InteractionPass::new(device, &scene.group0_layout, &scene.interaction_layout)?,
        relation_resolve: RelationResolvePass::new(device, &scene.relation_resolve_layout)?,
        label: LabelPass::new(
            device,
            &scene.group0_layout,
            &scene.label_declutter_layout,
            &scene.label_render_layout,
        )?,
        oit_composite: OitCompositePass::new(device)?,
        temporal: TemporalPass::new(device, &scene.group0_layout)?,
        bloom: plan
            .bloom()
            .map(|_| BloomPass::new(device, &scene.group0_layout))
            .transpose()?,
        motion_blur: plan
            .motion_blur()
            .map(|_| MotionBlurPass::new(device, &scene.group0_layout))
            .transpose()?,
        tonemap: TonemapPass::new(device, target_format, &scene.group0_layout)?,
        overlay: OverlayPass::new(
            device,
            target_format,
            &scene.group0_layout,
            &scene.overlay_layout,
        )?,
        trajectory: TrajectoryPass::new(device, &scene.trajectory_layout)?,
        occupancy: OccupancyPass::new(device, &scene.occupancy_layout)?,
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
                resource_memory_limit_bytes: config.resource_memory_limit_bytes,
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
                resource_memory_limit_bytes: config.resource_memory_limit_bytes,
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
        let scene_gpu = GpuScene::new(&device, config.residency, config.picking_page_capacity)?;
        let passes = realtime_passes(&device, target_format, &scene_gpu, &resolved_plan)?;
        let pass_nodes = realtime_nodes(
            resolved_plan.depth_of_field().is_some(),
            resolved_plan.bloom().is_some(),
            resolved_plan.motion_blur().is_some(),
        );
        let resources = realtime_resources();
        let order = graph::schedule(&pass_nodes)?;

        let profiler = GpuProfiler::new(&device, target_format)?;
        let picker = Picker::new(&device, config.picking_page_capacity)?;
        let chunk_residency = ChunkGpuResidency::new(&device, config.residency)?;
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
            temporal_scene_identity: None,
            mode: config.mode,
            profile,
            resolved_plan,
            focus_tracker: FocusTracker::default(),
            shadow_bound: ShadowBoundCache::default(),
            derived_cache: DerivedCache::new(config.derived_cache),
            derived_frame: 0,
            host_working_set: pdviewx_core::HostWorkingSet::new(config.source_budget),
            chunk_residency,
        })
    }
}

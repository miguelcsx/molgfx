//! Construction of persistent scene layouts, fallbacks, and empty tables.

use super::GpuScene;
use super::fallback::fallback_texture;
use crate::ResidencyConfig;
use crate::error::RenderError;
use crate::passes::{SurfaceComponentPass, SurfaceFieldPass};
use crate::scene_gpu::asset_arena::AssetArena;
use crate::scene_gpu::buffers::create_cull_tiles;
use crate::scene_gpu::instance_batch_table::GpuInstanceBatches;
use crate::scene_gpu::interaction_table::GpuInteractions;
use crate::scene_gpu::label_table::GpuLabels;
use crate::scene_gpu::layouts::{
    atom_cull_layout, attribute_timeline_layout, bond_cull_layout, cartoon_layout,
    generic_instance_cull_layout, generic_instance_render_layout, generic_point_cull_layout,
    generic_point_render_layout, instance_timeline_layout, interaction_layout,
    label_declutter_layout, label_render_layout, ligand_pose_layout, occupancy_layout,
    overlay_layout, primitive_layout, primitive_motion_layout, primitive_shadow_layout,
    quality_layout, relation_cull_layout, relation_resolve_layout, representation_layout,
    segmentation_layout, trajectory_layout, visual_cull_layout, volume_layout,
};
use crate::scene_gpu::ligand_pose_table::GpuLigandPoses;
use crate::scene_gpu::overlay_table::GpuOverlays;
use crate::scene_gpu::paged_bonds::PagedBondBatch;
use crate::scene_gpu::paged_chunks::PagedChunkBatch;
use crate::scene_gpu::point_batch_table::GpuPointBatches;
use crate::scene_gpu::primitive_table::GpuPrimitives;
use crate::scene_gpu::visual::VisualFallback;
use crate::scene_gpu::visual_parameters::VisualParameterTable;
use crate::scene_gpu::visual_programs::VisualProgramTable;
use crate::scene_gpu::visual_properties::VisualPropertyTable;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
    BufferDesc, BufferUsage, Device, ShaderStages, TextureFormat,
};

struct SceneFrameBinding<D: Device> {
    uniforms: D::Buffer,
    group: D::BindGroup,
    layout: D::BindGroupLayout,
}

/// The five surface-field pipeline layouts, created and read together.
struct SurfaceLayouts<D: Device> {
    output: D::BindGroupLayout,
    input: D::BindGroupLayout,
    erosion: D::BindGroupLayout,
    normal: D::BindGroupLayout,
    component: D::BindGroupLayout,
}

struct SurfaceInit<D: Device> {
    layouts: SurfaceLayouts<D>,
    field_texture: D::Texture,
    field: D::TextureView,
    normal_texture: D::Texture,
    normal: D::TextureView,
}

struct InitialLayouts<D: Device> {
    group2: D::BindGroupLayout,
    quality: D::BindGroupLayout,
    volume: D::BindGroupLayout,
    segmentation: D::BindGroupLayout,
    ribbon: D::BindGroupLayout,
    atom_cull: D::BindGroupLayout,
    bond_cull: D::BindGroupLayout,
    visual_cull: D::BindGroupLayout,
    interaction: D::BindGroupLayout,
    primitive: D::BindGroupLayout,
    primitive_motion: D::BindGroupLayout,
    primitive_shadow: D::BindGroupLayout,
    tail: TailLayouts<D>,
}

struct InitialResources<D: Device> {
    visual_properties: VisualPropertyTable<D>,
    visual_programs: VisualProgramTable<D>,
    visual_parameters: VisualParameterTable<D>,
    records: super::super::record_cache::RecordCache<D>,
    visibility: super::super::visibility_cache::VisibilityCache<D>,
    acceleration: super::super::acceleration_cache::AccelerationCache<D>,
    assets: AssetArena<D>,
    chunks: PagedChunkBatch<D>,
    bonds: PagedBondBatch<D>,
}

/// Everything a fresh scene is built from, created before assembly.
struct Prepared<D: Device> {
    frame_uniforms: D::Buffer,
    group0: D::BindGroup,
    group0_layout: D::BindGroupLayout,
    layouts: InitialLayouts<D>,
    surface: SurfaceInit<D>,
    cull_tiles: D::Buffer,
    cull_tiles_capacity: u64,
    initial: InitialResources<D>,
}

impl<D: Device> Prepared<D> {
    fn create(
        device: &D,
        config: ResidencyConfig,
        frame_resident_bytes: u64,
    ) -> Result<Self, RenderError> {
        let (cull_tiles, cull_tiles_capacity) = create_cull_tiles(device, 256)?;
        let frame = create_frame_binding(device, frame_resident_bytes)?;
        Ok(Self {
            frame_uniforms: frame.uniforms,
            group0: frame.group,
            group0_layout: frame.layout,
            layouts: initial_layouts(device)?,
            surface: surface_init(device)?,
            cull_tiles,
            cull_tiles_capacity,
            initial: initial_resources(device, config)?,
        })
    }
}

impl<D: Device> GpuScene<D> {
    pub(crate) fn new(
        device: &D,
        config: ResidencyConfig,
        picking_page_capacity: u32,
    ) -> Result<Self, RenderError> {
        let residency = super::residency_init::initialize(
            config,
            std::mem::size_of::<super::FrameUniforms>() as u64,
        )?;
        let prepared = Prepared::create(device, config, residency.frame_resident_bytes)?;
        assemble(device, config, prepared, residency, picking_page_capacity)
    }
    pub(crate) fn ensure_occupancy_layout(
        &mut self,
        device: &D,
        bounds_format: molgfx_gpu::TextureFormat,
    ) -> &D::BindGroupLayout {
        &self
            .occupancy
            .get_or_insert_with(|| (occupancy_layout(device, bounds_format), bounds_format))
            .0
    }
}

/// Assembles the persistent scene state from its prepared pieces.
fn assemble<D: Device>(
    device: &D,
    config: ResidencyConfig,
    prepared: Prepared<D>,
    residency: super::residency_init::InitialResidency,
    picking_page_capacity: u32,
) -> Result<GpuScene<D>, RenderError> {
    Ok(GpuScene {
        frame_uniforms: prepared.frame_uniforms,
        group0: prepared.group0,
        group0_layout: prepared.group0_layout,
        indirect: super::super::indirect_arena::IndirectArgsArena::new(),
        argument_offsets: std::collections::BTreeMap::new(),
        group2_layout: prepared.layouts.group2,
        quality_layout: prepared.layouts.quality,
        volume_layout: prepared.layouts.volume,
        segmentation_layout: prepared.layouts.segmentation,
        surface_field_output_layout: prepared.surface.layouts.output,
        surface_field_input_layout: prepared.surface.layouts.input,
        surface_field_erosion_layout: prepared.surface.layouts.erosion,
        surface_field_normal_layout: prepared.surface.layouts.normal,
        surface_component_layout: prepared.surface.layouts.component,
        ribbon_layout: prepared.layouts.ribbon,
        atom_cull_layout: prepared.layouts.atom_cull,
        bond_cull_layout: prepared.layouts.bond_cull,
        visual_cull_layout: prepared.layouts.visual_cull,
        cull_tiles: prepared.cull_tiles,
        cull_tiles_capacity: prepared.cull_tiles_capacity,
        cull_binding_revision: 0,
        cull_tile_count: 0,
        interaction_layout: prepared.layouts.interaction,
        relation_cull_layout: prepared.layouts.tail.relations.cull,
        relation_resolve_layout: prepared.layouts.tail.relations.resolve,
        generic_point_cull_layout: prepared.layouts.tail.relations.point_cull,
        generic_point_render_layout: prepared.layouts.tail.relations.point_render,
        generic_instance_cull_layout: prepared.layouts.tail.relations.instance_cull,
        generic_instance_render_layout: prepared.layouts.tail.relations.instance_render,
        instance_timeline_layout: prepared.layouts.tail.relations.instance_timeline,
        attribute_timeline_layout: prepared.layouts.tail.relations.attribute_timeline,
        primitive_layout: prepared.layouts.primitive,
        primitive_motion_layout: prepared.layouts.primitive_motion,
        primitive_shadow_layout: prepared.layouts.primitive_shadow,
        ligand_pose_layout: prepared.layouts.tail.ligand_pose,
        label_declutter_layout: prepared.layouts.tail.label_declutter,
        label_render_layout: prepared.layouts.tail.label_render,
        overlay_layout: prepared.layouts.tail.overlay,
        trajectory_layout: prepared.layouts.tail.trajectory,
        occupancy: None,
        surface_fields: crate::scene_gpu::surface_cache::SurfaceFieldCache::new(),
        _surface_field_fallback_texture: prepared.surface.field_texture,
        surface_field_fallback: prepared.surface.field,
        _surface_normal_fallback_texture: prepared.surface.normal_texture,
        surface_normal_fallback: prepared.surface.normal,
        asset_arena: prepared.initial.assets,
        assets: Vec::new(),
        structures: Vec::new(),
        slots: Vec::new(),
        volume_resources: Vec::new(),
        brick_atlases: Vec::new(),
        volume_slots: Vec::new(),
        mesh_slots: Vec::new(),
        mesh_synced: None,
        segmentation_resources: Vec::new(),
        segmentation_slots: Vec::new(),
        interactions: GpuInteractions::new(),
        point_batches: GpuPointBatches::new(),
        instance_batches: GpuInstanceBatches::new(),
        primitive: GpuPrimitives::new(),
        ligand_poses: GpuLigandPoses::new(),
        labels: GpuLabels::new(),
        overlays: GpuOverlays::new(),
        paged_chunks: prepared.initial.chunks,
        paged_bonds: prepared.initial.bonds,
        picking_pages: super::super::picking_pages::PickPages::new(picking_page_capacity)?,
        paged_instance_pick_scratch: Vec::with_capacity(config.machine_capacity),
        paged_relation_pick_scratch: Vec::with_capacity(config.machine_capacity),
        visual_properties: prepared.initial.visual_properties,
        visual_programs: prepared.initial.visual_programs,
        visual_parameters: prepared.initial.visual_parameters,
        visual_fallback: VisualFallback::new(device)?,
        specialized: crate::engine::pipeline_cache::SpecializedPipelines::new(),
        paged_visual_time_seconds: 0.0,
        paged_visual_time_revision: 0,
        scene_identity: None,
        structure_revision: None,
        slot_structure_revision: None,
        representation_revision: None,
        representation_membership_revision: None,
        volume_slot_revision: None,
        segmentation_slot_revision: None,
        records: prepared.initial.records,
        visibility: prepared.initial.visibility,
        acceleration: prepared.initial.acceleration,
        representation_scratch: Vec::new(),
        volume_handle_scratch: Vec::new(),
        segmentation_handle_scratch: Vec::new(),
        plan_scratch: Vec::new(),
        atom_scratch: Vec::new(),
        bond_scratch: Vec::new(),
        compaction_scratch: Vec::new(),
        ribbon_scratch: molgfx_geometry::RibbonMesh::default(),
        residency: residency.workspace,
        residency_machine: residency.machine,
        frame_residency_ticket: residency.frame_ticket,
        _frame_allocation: residency.frame_allocation,
        upload_fence: 0,
    })
}

fn initial_layouts<D: Device>(device: &D) -> Result<InitialLayouts<D>, RenderError> {
    Ok(InitialLayouts {
        group2: representation_layout(device)?,
        quality: quality_layout(device)?,
        volume: volume_layout(device),
        segmentation: segmentation_layout(device),
        ribbon: cartoon_layout(device),
        atom_cull: atom_cull_layout(device),
        bond_cull: bond_cull_layout(device),
        visual_cull: visual_cull_layout(device),
        interaction: interaction_layout(device),
        primitive: primitive_layout(device),
        primitive_motion: primitive_motion_layout(device),
        primitive_shadow: primitive_shadow_layout(device),
        tail: TailLayouts::new(device),
    })
}

/// Layouts belonging to no representation: the relation families and the
/// overlay, label, and trajectory families. Grouped because they share one
/// creation step and binding frequency.
struct TailLayouts<D: Device> {
    relations: RelationLayouts<D>,
    ligand_pose: D::BindGroupLayout,
    label_declutter: D::BindGroupLayout,
    label_render: D::BindGroupLayout,
    overlay: D::BindGroupLayout,
    trajectory: D::BindGroupLayout,
}

/// One layout per relation-resolver stage.
struct RelationLayouts<D: Device> {
    cull: D::BindGroupLayout,
    resolve: D::BindGroupLayout,
    point_cull: D::BindGroupLayout,
    point_render: D::BindGroupLayout,
    instance_cull: D::BindGroupLayout,
    instance_render: D::BindGroupLayout,
    instance_timeline: D::BindGroupLayout,
    attribute_timeline: D::BindGroupLayout,
}

impl<D: Device> TailLayouts<D> {
    fn new(device: &D) -> Self {
        Self {
            relations: RelationLayouts {
                cull: relation_cull_layout(device),
                resolve: relation_resolve_layout(device),
                point_cull: generic_point_cull_layout(device),
                point_render: generic_point_render_layout(device),
                instance_cull: generic_instance_cull_layout(device),
                instance_render: generic_instance_render_layout(device),
                instance_timeline: instance_timeline_layout(device),
                attribute_timeline: attribute_timeline_layout(device),
            },
            ligand_pose: ligand_pose_layout(device),
            label_declutter: label_declutter_layout(device),
            label_render: label_render_layout(device),
            overlay: overlay_layout(device),
            trajectory: trajectory_layout(device),
        }
    }
}

fn initial_resources<D: Device>(
    device: &D,
    config: ResidencyConfig,
) -> Result<InitialResources<D>, RenderError> {
    Ok(InitialResources {
        visual_properties: VisualPropertyTable::new(),
        visual_programs: VisualProgramTable::new(),
        visual_parameters: VisualParameterTable::new(),
        records: super::super::record_cache::RecordCache::new(),
        visibility: super::super::visibility_cache::VisibilityCache::new(),
        acceleration: super::super::acceleration_cache::AccelerationCache::new(),
        assets: AssetArena::new(device)?,
        chunks: PagedChunkBatch::new(device, config)?,
        bonds: PagedBondBatch::new(device, config)?,
    })
}

fn surface_init<D: Device>(device: &D) -> Result<SurfaceInit<D>, RenderError> {
    let (field_texture, field) =
        fallback_texture(device, "unused surface field", TextureFormat::R32Float)?;
    let (normal_texture, normal) =
        fallback_texture(device, "unused surface normals", TextureFormat::Rgba8Snorm)?;
    Ok(SurfaceInit {
        layouts: SurfaceLayouts {
            output: SurfaceFieldPass::<D>::output_layout(device),
            input: SurfaceFieldPass::<D>::input_layout(device),
            erosion: SurfaceFieldPass::<D>::erosion_layout(device),
            normal: SurfaceFieldPass::<D>::normal_layout(device),
            component: SurfaceComponentPass::<D>::layout(device),
        },
        field_texture,
        field,
        normal_texture,
        normal,
    })
}

fn create_frame_binding<D: Device>(
    device: &D,
    resident_bytes: u64,
) -> Result<SceneFrameBinding<D>, RenderError> {
    let uniforms = device.create_buffer(&BufferDesc {
        label: "frame uniforms",
        size: resident_bytes,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group0: per-frame",
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX
                .union(ShaderStages::FRAGMENT)
                .union(ShaderStages::COMPUTE),
            ty: BindingType::Uniform,
        }],
    });
    let group = device.create_bind_group(&BindGroupDesc {
        label: "group0: per-frame",
        layout: &layout,
        entries: &[BindGroupEntry::Buffer {
            binding: 0,
            buffer: &uniforms,
        }],
    });
    Ok(SceneFrameBinding {
        uniforms,
        group,
        layout,
    })
}

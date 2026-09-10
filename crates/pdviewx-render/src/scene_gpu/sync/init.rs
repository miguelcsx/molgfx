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
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
    BufferDesc, BufferUsage, Device, ShaderStages, TextureFormat,
};

struct SceneFrameBinding<D: Device> {
    uniforms: D::Buffer,
    group: D::BindGroup,
    layout: D::BindGroupLayout,
}

struct SurfaceInit<D: Device> {
    output_layout: D::BindGroupLayout,
    input_layout: D::BindGroupLayout,
    erosion_layout: D::BindGroupLayout,
    normal_layout: D::BindGroupLayout,
    component_layout: D::BindGroupLayout,
    field_texture: D::Texture,
    field: D::TextureView,
    normal_texture: D::Texture,
    normal: D::TextureView,
}

impl<D: Device> GpuScene<D> {
    #[allow(clippy::too_many_lines)]
    pub fn new(
        device: &D,
        config: ResidencyConfig,
        picking_page_capacity: u32,
    ) -> Result<Self, RenderError> {
        let frame_bytes = std::mem::size_of::<super::FrameUniforms>() as u64;
        let residency = super::residency_init::initialize(config, frame_bytes)?;
        let frame = create_frame_binding(device, residency.frame_resident_bytes)?;
        let group2_layout = representation_layout(device)?;
        let quality_layout = quality_layout(device)?;
        let (volume_layout, segmentation_layout) =
            (volume_layout(device), segmentation_layout(device));
        let surface = surface_init(device)?;
        let ribbon_layout = cartoon_layout(device);
        let atom_cull_layout = atom_cull_layout(device);
        let bond_cull_layout = bond_cull_layout(device);
        let visual_cull_layout = visual_cull_layout(device);
        let (cull_tiles, cull_tiles_capacity) = create_cull_tiles(device, 256)?;
        let interaction_layout = interaction_layout(device);
        let primitive_layout = primitive_layout(device);
        let primitive_motion_layout = primitive_motion_layout(device);
        let primitive_shadow_layout = primitive_shadow_layout(device);
        let asset_arena = AssetArena::new(device)?;
        let paged_chunks = PagedChunkBatch::new(device, config)?;
        let paged_bonds = PagedBondBatch::new(device, config)?;
        Ok(Self {
            frame_uniforms: frame.uniforms,
            group0: frame.group,
            group0_layout: frame.layout,
            group2_layout,
            quality_layout,
            volume_layout,
            segmentation_layout,
            surface_field_output_layout: surface.output_layout,
            surface_field_input_layout: surface.input_layout,
            surface_field_erosion_layout: surface.erosion_layout,
            surface_field_normal_layout: surface.normal_layout,
            surface_component_layout: surface.component_layout,
            ribbon_layout,
            atom_cull_layout,
            bond_cull_layout,
            visual_cull_layout,
            cull_tiles,
            cull_tiles_capacity,
            cull_binding_revision: 0,
            cull_tile_count: 0,
            interaction_layout,
            relation_cull_layout: relation_cull_layout(device),
            relation_resolve_layout: relation_resolve_layout(device),
            generic_point_cull_layout: generic_point_cull_layout(device),
            generic_point_render_layout: generic_point_render_layout(device),
            generic_instance_cull_layout: generic_instance_cull_layout(device),
            generic_instance_render_layout: generic_instance_render_layout(device),
            instance_timeline_layout: instance_timeline_layout(device),
            attribute_timeline_layout: attribute_timeline_layout(device),
            primitive_layout,
            primitive_motion_layout,
            primitive_shadow_layout,
            ligand_pose_layout: ligand_pose_layout(device),
            label_declutter_layout: label_declutter_layout(device),
            label_render_layout: label_render_layout(device),
            overlay_layout: overlay_layout(device),
            trajectory_layout: trajectory_layout(device),
            occupancy_layout: occupancy_layout(device),
            _surface_field_fallback_texture: surface.field_texture,
            surface_field_fallback: surface.field,
            _surface_normal_fallback_texture: surface.normal_texture,
            surface_normal_fallback: surface.normal,
            asset_arena,
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
            paged_chunks,
            paged_bonds,
            picking_pages: super::super::picking_pages::PickPages::new(picking_page_capacity)?,
            paged_instance_pick_scratch: Vec::with_capacity(config.machine_capacity),
            paged_relation_pick_scratch: Vec::with_capacity(config.machine_capacity),
            visual_properties: VisualPropertyTable::new(),
            visual_programs: VisualProgramTable::new(),
            visual_parameters: VisualParameterTable::new(),
            visual_fallback: VisualFallback::new(device)?,
            paged_visual_time_seconds: 0.0,
            paged_visual_time_revision: 0,
            scene_identity: None,
            structure_revision: None,
            slot_structure_revision: None,
            representation_revision: None,
            volume_slot_revision: None,
            segmentation_slot_revision: None,
            representation_scratch: Vec::new(),
            volume_handle_scratch: Vec::new(),
            segmentation_handle_scratch: Vec::new(),
            plan_scratch: Vec::new(),
            atom_scratch: Vec::new(),
            bond_scratch: Vec::new(),
            compaction_scratch: Vec::new(),
            ribbon_scratch: pdviewx_geometry::RibbonMesh::default(),
            residency: residency.workspace,
            residency_machine: residency.machine,
            frame_residency_ticket: residency.frame_ticket,
            _frame_allocation: residency.frame_allocation,
            upload_fence: 0,
        })
    }
}

fn surface_init<D: Device>(device: &D) -> Result<SurfaceInit<D>, RenderError> {
    let (field_texture, field) =
        fallback_texture(device, "unused surface field", TextureFormat::R32Float)?;
    let (normal_texture, normal) =
        fallback_texture(device, "unused surface normals", TextureFormat::Rgba8Snorm)?;
    Ok(SurfaceInit {
        output_layout: SurfaceFieldPass::<D>::output_layout(device),
        input_layout: SurfaceFieldPass::<D>::input_layout(device),
        erosion_layout: SurfaceFieldPass::<D>::erosion_layout(device),
        normal_layout: SurfaceFieldPass::<D>::normal_layout(device),
        component_layout: SurfaceComponentPass::<D>::layout(device),
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

//! Revision-diffed synchronization of persistent scene GPU state.
//!
//! Reconciliation is `O(structures + representation slots)` after scene
//! edits. An unchanged frame performs no scene uploads or allocations.

mod draws;
mod fallback;
mod properties;
mod segmentations;
mod trajectories;
mod volumes;

use self::fallback::fallback_texture;
use super::interaction_table::GpuInteractions;
use super::label_table::GpuLabels;
use super::layouts::{
    cartoon_layout, cull_layout, interaction_layout, label_declutter_layout, label_render_layout,
    overlay_layout, primitive_layout, primitive_motion_layout, representation_layout,
    segmentation_layout, trajectory_layout, volume_layout,
};
use super::overlay_table::GpuOverlays;
use super::primitive_table::GpuPrimitives;
use super::segmentation_slot::{GpuSegmentationResource, GpuSegmentationSlot};
use super::slot_types::{SlotKey, SlotPlan, SlotSync};
use super::slots::GpuSlot;
use super::structure::GpuStructure;
use super::uniforms::FrameUniforms;
use super::volume_slot::{GpuVolumeResource, GpuVolumeSlot};
use crate::error::RenderError;
use crate::passes::SurfaceFieldPass;
use pdviewx_core::{
    AtomGpu, BondGpu, RepresentationHandle, Scene, SegmentationHandle, VolumeHandle,
};
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
    BufferDesc, BufferUsage, Device, Queue, ShaderStages, TextureFormat,
};

/// GPU-resident scene state with stable structure and representation slots.
#[derive(Debug)]
pub struct GpuScene<D: Device> {
    pub frame_uniforms: D::Buffer,
    pub group0: D::BindGroup,
    pub group0_layout: D::BindGroupLayout,
    pub group2_layout: D::BindGroupLayout,
    pub volume_layout: D::BindGroupLayout,
    pub segmentation_layout: D::BindGroupLayout,
    pub surface_field_output_layout: D::BindGroupLayout,
    pub surface_field_input_layout: D::BindGroupLayout,
    pub surface_field_erosion_layout: D::BindGroupLayout,
    pub ribbon_layout: D::BindGroupLayout,
    pub cull_layout: D::BindGroupLayout,
    pub interaction_layout: D::BindGroupLayout,
    pub primitive_layout: D::BindGroupLayout,
    pub primitive_motion_layout: D::BindGroupLayout,
    pub label_declutter_layout: D::BindGroupLayout,
    pub label_render_layout: D::BindGroupLayout,
    pub overlay_layout: D::BindGroupLayout,
    pub trajectory_layout: D::BindGroupLayout,
    _surface_field_fallback_texture: D::Texture,
    surface_field_fallback: D::TextureView,
    _surface_provenance_fallback_texture: D::Texture,
    surface_provenance_fallback: D::TextureView,
    structures: Vec<GpuStructure<D>>,
    slots: Vec<GpuSlot<D>>,
    volume_resources: Vec<GpuVolumeResource<D>>,
    volume_slots: Vec<GpuVolumeSlot<D>>,
    mesh_slots: Vec<super::mesh_slot::GpuMeshSlot<D>>,
    mesh_synced: Option<(u64, u64)>,
    segmentation_resources: Vec<GpuSegmentationResource<D>>,
    segmentation_slots: Vec<GpuSegmentationSlot<D>>,
    interactions: GpuInteractions<D>,
    primitive: GpuPrimitives<D>,
    labels: GpuLabels<D>,
    overlays: GpuOverlays<D>,
    structure_revision: Option<u64>,
    slot_structure_revision: Option<u64>,
    representation_revision: Option<u64>,
    volume_slot_revision: Option<(u64, u64)>,
    segmentation_slot_revision: Option<(u64, u64)>,
    representation_scratch: Vec<(u16, RepresentationHandle)>,
    volume_handle_scratch: Vec<VolumeHandle>,
    segmentation_handle_scratch: Vec<SegmentationHandle>,
    plan_scratch: Vec<SlotPlan>,
    atom_scratch: Vec<AtomGpu>,
    bond_scratch: Vec<BondGpu>,
    compaction_scratch: Vec<u32>,
    ribbon_scratch: pdviewx_geometry::RibbonMesh,
}

impl<D: Device> GpuScene<D> {
    pub fn new(device: &D) -> Result<Self, RenderError> {
        let frame_uniforms = device.create_buffer(&BufferDesc {
            label: "frame uniforms",
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
        })?;
        let group0_layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group0: per-frame",
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX
                    .union(ShaderStages::FRAGMENT)
                    .union(ShaderStages::COMPUTE),
                ty: BindingType::Uniform,
            }],
        });
        let group0 = device.create_bind_group(&BindGroupDesc {
            label: "group0: per-frame",
            layout: &group0_layout,
            entries: &[BindGroupEntry::Buffer {
                binding: 0,
                buffer: &frame_uniforms,
            }],
        });
        let group2_layout = representation_layout(device);
        let volume_layout = volume_layout(device);
        let segmentation_layout = segmentation_layout(device);
        let surface_field_output_layout = SurfaceFieldPass::<D>::output_layout(device);
        let surface_field_input_layout = SurfaceFieldPass::<D>::input_layout(device);
        let surface_field_erosion_layout = SurfaceFieldPass::<D>::erosion_layout(device);
        let (surface_field_fallback_texture, surface_field_fallback) =
            fallback_texture(device, "unused surface field", TextureFormat::R32Float)?;
        let (surface_provenance_fallback_texture, surface_provenance_fallback) =
            fallback_texture(device, "unused surface provenance", TextureFormat::R32Uint)?;
        let ribbon_layout = cartoon_layout(device);
        let cull_layout = cull_layout(device);
        let interaction_layout = interaction_layout(device);
        let primitive_layout = primitive_layout(device);
        let primitive_motion_layout = primitive_motion_layout(device);
        let label_declutter_layout = label_declutter_layout(device);
        let label_render_layout = label_render_layout(device);
        let overlay_layout = overlay_layout(device);
        let trajectory_layout = trajectory_layout(device);
        Ok(Self {
            frame_uniforms,
            group0,
            group0_layout,
            group2_layout,
            volume_layout,
            segmentation_layout,
            surface_field_output_layout,
            surface_field_input_layout,
            surface_field_erosion_layout,
            ribbon_layout,
            cull_layout,
            interaction_layout,
            primitive_layout,
            primitive_motion_layout,
            label_declutter_layout,
            label_render_layout,
            overlay_layout,
            trajectory_layout,
            _surface_field_fallback_texture: surface_field_fallback_texture,
            surface_field_fallback,
            _surface_provenance_fallback_texture: surface_provenance_fallback_texture,
            surface_provenance_fallback,
            structures: Vec::new(),
            slots: Vec::new(),
            volume_resources: Vec::new(),
            volume_slots: Vec::new(),
            mesh_slots: Vec::new(),
            mesh_synced: None,
            segmentation_resources: Vec::new(),
            segmentation_slots: Vec::new(),
            interactions: GpuInteractions::new(),
            primitive: GpuPrimitives::new(),
            labels: GpuLabels::new(),
            overlays: GpuOverlays::new(),
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
        })
    }

    pub fn write_frame_uniforms(&self, queue: &D::Queue, uniforms: &FrameUniforms) {
        queue.write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(uniforms));
    }

    /// Rebuilds resident caller meshes when the mesh table or the structures
    /// they hang from change. Untouched meshes keep their buffers.
    fn sync_mesh_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let revision = (scene.mesh_revision(), scene.structure_revision());
        if self.mesh_synced == Some(revision) {
            return Ok(false);
        }
        self.mesh_slots.clear();
        for (mesh_handle, mesh) in scene.meshes() {
            if !mesh.visible() {
                continue;
            }
            let Some(structure) = self
                .structures
                .iter()
                .find(|structure| structure.handle == mesh.owner())
            else {
                continue;
            };
            let mut slot = super::mesh_slot::GpuMeshSlot::new();
            let instances: Vec<_> = scene.mesh_instance_transforms(mesh_handle).collect();
            slot.sync(
                device,
                queue,
                mesh,
                &self.ribbon_layout,
                structure,
                super::mesh_slot::MeshOccurrences {
                    transforms: &instances,
                    model_to_world: scene
                        .structure(mesh.owner())
                        .map_or(pdviewx_math::Mat4::IDENTITY, |placed| placed.model_to_world),
                    entity: pdviewx_core::EntityId::pack(
                        pdviewx_core::EntityKind::Mesh,
                        pdviewx_core::Scene::mesh_row(mesh_handle),
                    ),
                },
            )?;
            self.mesh_slots.push(slot);
        }
        self.mesh_synced = Some(revision);
        Ok(true)
    }

    pub fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = self.structure_revision != Some(scene.structure_revision())
            || self.representation_revision != Some(scene.representation_revision())
            || self.volume_slot_revision
                != Some((scene.volume_revision(), scene.representation_revision()))
            || self.segmentation_slot_revision
                != Some((
                    scene.segmentation_revision(),
                    scene.representation_revision(),
                ));
        self.reconcile_structures(scene);
        changed |= self.sync_semantic_tables(device, queue, scene)?;
        self.reconcile_slots(scene);
        self.reconcile_volume_slots(scene);
        self.reconcile_segmentations(scene);
        changed |= self.sync_volume_resources(device, queue, scene)?;
        changed |= self.sync_segmentation_resources(device, queue, scene)?;
        for gpu in &mut self.structures {
            if let Some(placed) = scene.structure(gpu.handle) {
                changed |= gpu.sync(device, queue, placed, &self.trajectory_layout)?;
            }
        }
        for slot in &mut self.slots {
            let Some(placed) = scene.structure(slot.key.structure) else {
                continue;
            };
            let Some(representation) = scene.representation(slot.key.representation) else {
                continue;
            };
            let Some(selection_handle) = representation.selection() else {
                continue;
            };
            let Some(selection) = scene.selection_for(selection_handle, slot.key.structure) else {
                continue;
            };
            let Some(representation_revision) =
                scene.representation_content_revision(slot.key.representation)
            else {
                continue;
            };
            let Some(structure_gpu) = self.structures.get(slot.structure_index) else {
                continue;
            };
            let (color_property, appearance_property, property_revisions) =
                properties::resolve(scene, representation, slot.key.structure);
            let (overlay_volume, overlay_view, overlay_binding_revision) =
                super::scalar_overlay::resolve(
                    &self.volume_resources,
                    scene,
                    representation,
                    &self.surface_field_fallback,
                );
            changed |= slot.sync(SlotSync {
                device,
                queue,
                layout: &self.group2_layout,
                ribbon_layout: &self.ribbon_layout,
                cull_layout: &self.cull_layout,
                surface_field_output_layout: &self.surface_field_output_layout,
                surface_field_input_layout: &self.surface_field_input_layout,
                surface_field_erosion_layout: &self.surface_field_erosion_layout,
                surface_field_fallback: &self.surface_field_fallback,
                surface_provenance_fallback: &self.surface_provenance_fallback,
                overlay_volume,
                overlay_view,
                overlay_binding_revision,
                frame: &self.frame_uniforms,
                structure_gpu,
                placed,
                representation,
                representation_revision,
                selection,
                color_property,
                appearance_property,
                property_revisions,
                atoms: &mut self.atom_scratch,
                bonds: &mut self.bond_scratch,
                compaction: &mut self.compaction_scratch,
                ribbon: &mut self.ribbon_scratch,
            })?;
        }
        changed |= self.sync_volume_slots(device, queue, scene)?;
        changed |= self.sync_mesh_slots(device, queue, scene)?;
        changed |= self.sync_segmentation_slots(device, queue, scene)?;
        Ok(changed)
    }

    fn sync_semantic_tables(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = self.interactions.sync(
            device,
            queue,
            &self.interaction_layout,
            scene,
            &self.structures,
        )?;
        changed |= self.primitive.sync(
            device,
            queue,
            &self.primitive_layout,
            &self.primitive_motion_layout,
            scene,
            &self.structures,
        )?;
        changed |= self.labels.sync(
            device,
            queue,
            &self.label_declutter_layout,
            &self.label_render_layout,
            scene,
            &self.structures,
        )?;
        changed |= self
            .overlays
            .sync(device, queue, &self.overlay_layout, scene)?;
        Ok(changed)
    }

    fn reconcile_structures(&mut self, scene: &Scene) {
        let revision = scene.structure_revision();
        if self.structure_revision == Some(revision) {
            return;
        }
        let mut old = std::mem::take(&mut self.structures);
        for (structure_id, (handle, _)) in scene.structures().enumerate() {
            let structure_id = u32::try_from(structure_id).map_or(u32::MAX, |value| value);
            if let Some(index) = old.iter().position(|gpu| gpu.handle == handle) {
                let mut structure = old.swap_remove(index);
                structure.set_structure_id(structure_id);
                self.structures.push(structure);
            } else {
                self.structures
                    .push(GpuStructure::new(handle, structure_id));
            }
        }
        self.structure_revision = Some(revision);
    }

    fn reconcile_slots(&mut self, scene: &Scene) {
        let revision = scene.representation_revision();
        if self.representation_revision == Some(revision)
            && self.slot_structure_revision == Some(scene.structure_revision())
        {
            return;
        }
        self.representation_scratch.clear();
        self.representation_scratch.extend(
            scene
                .representations()
                .filter(|(_, rep)| rep.visible)
                .filter(|(_, rep)| rep.selection().is_some())
                .map(|(handle, rep)| (rep.order, handle)),
        );
        self.representation_scratch.sort_unstable();
        self.plan_scratch.clear();
        for (_, representation) in &self.representation_scratch {
            for (structure_index, structure) in self.structures.iter().enumerate() {
                let Some(value) = scene.representation(*representation) else {
                    continue;
                };
                let Some(selection) = value.selection() else {
                    continue;
                };
                let Some(selection) = scene.selection_for(selection, structure.handle) else {
                    continue;
                };
                let Some(placed) = scene.structure(structure.handle) else {
                    continue;
                };
                if selection.count(placed.atoms.len()) == 0 {
                    continue;
                }
                self.plan_scratch.push(SlotPlan {
                    key: SlotKey {
                        structure: structure.handle,
                        representation: *representation,
                    },
                    structure_index,
                });
            }
        }
        let mut old = std::mem::take(&mut self.slots);
        for plan in &self.plan_scratch {
            if let Some(index) = old.iter().position(|slot| slot.key == plan.key) {
                let mut slot = old.swap_remove(index);
                slot.structure_index = plan.structure_index;
                self.slots.push(slot);
            } else {
                self.slots.push(GpuSlot::new(*plan));
            }
        }
        self.representation_revision = Some(revision);
        self.slot_structure_revision = Some(scene.structure_revision());
    }

    fn reconcile_volume_slots(&mut self, scene: &Scene) {
        let revision = (scene.volume_revision(), scene.representation_revision());
        if self.volume_slot_revision == Some(revision) {
            return;
        }
        self.representation_scratch.clear();
        self.representation_scratch.extend(
            scene
                .representations()
                .filter(|(_, representation)| {
                    representation.visible && representation.volume_handle().is_some()
                })
                .map(|(handle, representation)| (representation.order, handle)),
        );
        self.representation_scratch.sort_unstable();
        self.volume_handle_scratch.clear();
        self.volume_handle_scratch.extend(
            scene
                .representations()
                .filter(|(_, representation)| representation.visible)
                .filter_map(|(_, representation)| {
                    representation
                        .volume_handle()
                        .or_else(|| representation.surface_scalar.map(|overlay| overlay.field))
                }),
        );
        self.volume_handle_scratch.sort_unstable();
        self.volume_handle_scratch.dedup();
        let mut old_resources = std::mem::take(&mut self.volume_resources);
        for handle in &self.volume_handle_scratch {
            if let Some(index) = old_resources
                .iter()
                .position(|resource| resource.handle == *handle)
            {
                self.volume_resources.push(old_resources.swap_remove(index));
            } else {
                self.volume_resources.push(GpuVolumeResource::new(*handle));
            }
        }
        let mut old = std::mem::take(&mut self.volume_slots);
        for (_, handle) in &self.representation_scratch {
            if let Some(index) = old.iter().position(|slot| slot.representation == *handle) {
                self.volume_slots.push(old.swap_remove(index));
            } else {
                self.volume_slots.push(GpuVolumeSlot::new(*handle));
            }
        }
        self.volume_slot_revision = Some(revision);
    }
}

//! Building the frame's shared surface fields before any slot binds.
//!
//! A field is created while the field cache is borrowed exclusively, and the
//! binding pass only ever reads that cache. Every field the frame needs must
//! therefore exist before binding starts, which is why resolution is its own
//! pass rather than something each slot does as it binds.
//!
//! The pass also resolves the per-slot state the key is built from: the
//! selection bounds, the representation uniforms and the identity of the shared
//! records. Those are the same inputs the binding pass uses, so the key cannot
//! disagree with the uniforms its field is generated under.

use super::super::record_cache::{RecordCache, RecordSet};
use super::super::scalar_overlay;
use super::super::slots::GpuSlot;
use super::super::surface_field::FieldSync;
use super::super::uniforms::RepresentationUniforms;
use super::GpuScene;
use crate::error::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    /// Resolves each slot's field key and builds every field the frame needs.
    ///
    /// A key shared by four surfaces builds one field: the cache keys on
    /// geometry and sampling policy, so the second surface through finds it
    /// already resident and allocates nothing.
    pub(super) fn prepare_surface_fields(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        quality: bool,
    ) -> Result<(), RenderError> {
        // Destructured so the slot list can be walked mutably while the shared
        // tables stay borrowed immutably, exactly as the binding pass does.
        let slots = &mut self.slots;
        let records = &self.records;
        let structures = &self.structures;
        let volume_resources = &self.volume_resources;
        let fallback = &self.surface_field_fallback;
        let fields = &mut self.surface_fields;
        for slot in slots.iter_mut() {
            let Some(representation) = scene.representation(slot.key.representation) else {
                continue;
            };
            let Some(placed) = scene.structure(slot.key.structure) else {
                continue;
            };
            let Some(selection_handle) = representation.selection() else {
                continue;
            };
            let Some(structure_gpu) = structures.get(slot.structure_index) else {
                continue;
            };
            let Some(selection) = scene.selection_for(selection_handle, slot.key.structure) else {
                continue;
            };
            let (_, _, property_revisions) =
                super::properties::resolve(scene, representation, slot.key.structure);
            let record_key = RecordCache::<D>::key(
                scene,
                placed,
                slot.key.structure,
                representation,
                selection_handle,
                structure_gpu.asset_identity(),
                property_revisions,
            );
            let Some(set) = records.get(record_key).and_then(RecordSet::as_ref) else {
                continue;
            };
            let (atoms, compaction, atom_count) = (set.atoms, set.compaction, set.atom_count);
            let selection_bounds =
                slot.selection_bounds
                    .resolve(placed, representation, selection)?;
            let (overlay_volume, _, _) =
                scalar_overlay::resolve(volume_resources, scene, representation, fallback);
            let uniforms = RepresentationUniforms::for_quality(
                representation,
                selection_bounds,
                overlay_volume,
                quality,
            );
            // The colour column is resolved from the same arena the visual
            // programs sample, so a property scheme adds no second upload path.
            if let Some(handle) = representation.color.property_handle() {
                slot.adopt_color_column(self.visual_properties.color_column(handle));
            }
            slot.surface
                .resolve_key(&uniforms, representation, record_key.geometry(), atom_count);
            let Some(key) = slot.surface.key() else {
                continue;
            };
            fields.prepare(&FieldSync {
                device,
                queue,
                output_layout: &self.surface_field_output_layout,
                input_layout: &self.surface_field_input_layout,
                erosion_layout: &self.surface_field_erosion_layout,
                normal_layout: &self.surface_field_normal_layout,
                component_layout: &self.surface_component_layout,
                key,
                policy: representation.params.surface_components,
                dimensions: [
                    uniforms.grid_size[0],
                    uniforms.grid_size[1],
                    uniforms.grid_size[2],
                ],
                atoms,
                structure: structure_gpu,
                asset_arena: &self.asset_arena,
                compaction,
                uniforms: &uniforms,
            })?;
        }
        Ok(())
    }

    /// Releases every field no surface asked for, and charges the rest.
    ///
    /// The ledger is charged against the first resident key, because a field
    /// cache is one scene-wide allocation shared by every surface rather than
    /// one resource per representation.
    pub(super) fn retain_surface_fields(
        &mut self,
        derived_cache: &mut crate::DerivedCache,
        derived_frame: u64,
    ) {
        let live: Vec<_> = self.slots.iter().filter_map(GpuSlot::surface_key).collect();
        self.surface_fields.retain(&live);
        let Some(first) = live.first().copied() else {
            return;
        };
        let _ = derived_cache.retain(
            crate::DerivedCacheKey::SurfaceField(first),
            crate::DerivedCacheClass::SurfaceField,
            crate::DerivedFootprint {
                cpu_bytes: 0,
                gpu_bytes: self.surface_fields.resident_bytes(),
            },
            derived_frame,
        );
    }
}

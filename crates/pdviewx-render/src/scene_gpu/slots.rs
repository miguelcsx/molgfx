//! Persistent structure allocations and drawable representation slots.

mod bindings;
mod draws;

use super::buffers::{
    count, ensure_indices, upload_grow, write_args, write_counts, write_draw_args,
};
use super::ribbon_slot::{RibbonSlot, RibbonSync};
use super::slot_types::{
    RecordState, RecordUpload, RibbonState, SlotKey, SlotPlan, SlotShading, SlotSync, SlotSynced,
};
use super::surface_slot::{SurfaceSlot, SurfaceSync};
use super::uniforms::{RepresentationUniforms, write_representation_uniforms};
use crate::error::RenderError;
use pdviewx_core::{RepresentationKind, SurfaceKind};
use pdviewx_gpu::{BufferDesc, BufferUsage, Device};

#[derive(Debug)]
pub(super) struct GpuSlot<D: Device> {
    pub(super) key: SlotKey,
    pub(super) structure_index: usize,
    atoms: Option<D::Buffer>,
    bonds: Option<D::Buffer>,
    atom_args: Option<D::Buffer>,
    bond_args: Option<D::Buffer>,
    surface_args: Option<D::Buffer>,
    compaction: Option<D::Buffer>,
    representation_uniforms: Option<D::Buffer>,
    surface: SurfaceSlot<D>,
    group2: Option<D::BindGroup>,
    cull_group: Option<D::BindGroup>,
    visible_atoms: Option<D::Buffer>,
    visible_bonds: Option<D::Buffer>,
    atom_visibility: Option<D::Buffer>,
    counts: Option<D::Buffer>,
    atom_count: u32,
    bond_count: u32,
    atoms_capacity: u64,
    bonds_capacity: u64,
    compaction_capacity: u64,
    visible_atoms_capacity: u64,
    visible_bonds_capacity: u64,
    atom_visibility_capacity: u64,
    synced: Option<SlotSynced>,
    translucent: bool,
    kind: RepresentationKind,
    shading: SlotShading,
    ribbon: RibbonSlot<D>,
}
impl<D: Device> GpuSlot<D> {
    pub(super) fn new(plan: SlotPlan) -> Self {
        Self {
            key: plan.key,
            structure_index: plan.structure_index,
            atoms: None,
            bonds: None,
            atom_args: None,
            bond_args: None,
            surface_args: None,
            compaction: None,
            representation_uniforms: None,
            surface: SurfaceSlot::new(),
            group2: None,
            cull_group: None,
            visible_atoms: None,
            visible_bonds: None,
            atom_visibility: None,
            counts: None,
            atom_count: 0,
            bond_count: 0,
            atoms_capacity: 0,
            bonds_capacity: 0,
            compaction_capacity: 0,
            visible_atoms_capacity: 0,
            visible_bonds_capacity: 0,
            atom_visibility_capacity: 0,
            synced: None,
            translucent: false,
            shading: SlotShading::default(),
            kind: RepresentationKind::Spacefill,
            ribbon: RibbonSlot::new(),
        }
    }

    pub(super) fn sync(&mut self, mut input: SlotSync<'_, D>) -> Result<bool, RenderError> {
        let table = &input.placed.atoms;
        let current = SlotSynced {
            representation: input.representation_revision,
            records: RecordState::new(input.representation),
            ribbon: RibbonState::new(input.representation),
            color: table.color().revision().get(),
            flags: table.flags().revision().get(),
            semantic: table.semantic().revision().get(),
            properties: input.property_revisions,
            secondary_structure: input.placed.secondary_structure.revision().get(),
            structure_binding: input.structure_gpu.binding_revision,
            coordinates: [
                input.placed.atoms.coords().generation(),
                input.placed.trajectory_revision(),
            ],
            spatial_bounds: [
                input.placed.atoms.coords().generation(),
                input.placed.trajectory_pair_revision(),
            ],
            overlay_binding: input.overlay_binding_revision,
        };
        if self.synced == Some(current) {
            return Ok(false);
        }
        self.translucent = input.representation.is_translucent();
        self.kind = input.representation.kind;
        self.shading = SlotShading {
            clipped: !input.representation.clipping.planes().is_empty(),
            surface_grid: matches!(
                input.representation.params.surface_kind,
                SurfaceKind::SolventExcluded | SurfaceKind::Gaussian
            ),
        };
        let representation_changed = self
            .synced
            .is_none_or(|old| old.representation != current.representation);
        let records_changed = self.synced.is_none_or(|old| {
            old.records != current.records
                || old.color != current.color
                || old.flags != current.flags
                || old.semantic != current.semantic
                || old.properties != current.properties
        });
        if is_spline(input.representation.kind) {
            self.sync_cartoon(&mut input, &current, representation_changed)?;
        } else if records_changed {
            self.ribbon.clear();
            self.upload_records(&mut input.records())?;
        } else if self.kind == RepresentationKind::Surface
            && (representation_changed
                || self.synced.is_none_or(|old| {
                    old.coordinates != current.coordinates
                        || old.overlay_binding != current.overlay_binding
                }))
        {
            let coordinates_changed = self
                .synced
                .is_none_or(|old| old.coordinates != current.coordinates);
            self.sync_surface_resources(&input, coordinates_changed)?;
            self.bind(
                input.device,
                input.layout,
                input.structure_gpu,
                input.surface_field_fallback,
                input.surface_provenance_fallback,
                input.overlay_view,
            );
        } else if self.group2.is_none()
            || self.synced.is_none_or(|old| {
                old.structure_binding != current.structure_binding
                    || old.overlay_binding != current.overlay_binding
            })
        {
            self.bind(
                input.device,
                input.layout,
                input.structure_gpu,
                input.surface_field_fallback,
                input.surface_provenance_fallback,
                input.overlay_view,
            );
        }
        if self.cull_group.is_none()
            || self
                .synced
                .is_some_and(|old| old.structure_binding != current.structure_binding)
        {
            self.bind_cull(
                input.device,
                input.cull_layout,
                input.structure_gpu,
                input.frame,
            );
        }
        self.sync_uniforms(&input, &current, representation_changed);
        self.synced = Some(current);
        Ok(true)
    }

    fn sync_cartoon(
        &mut self,
        input: &mut SlotSync<'_, D>,
        current: &SlotSynced,
        representation_changed: bool,
    ) -> Result<(), RenderError> {
        self.atom_count = 0;
        self.bond_count = 0;
        let geometry_changed = self.synced.is_none_or(|old| {
            old.ribbon != current.ribbon
                || old.color != current.color
                || old.flags != current.flags
                || old.semantic != current.semantic
                || old.properties != current.properties
                || old.coordinates != current.coordinates
                || old.secondary_structure != current.secondary_structure
        });
        if geometry_changed {
            self.ribbon.sync(&mut RibbonSync {
                device: input.device,
                queue: input.queue,
                layout: input.ribbon_layout,
                structure: input.structure_gpu,
                placed: input.placed,
                representation: input.representation,
                selection: input.selection,
                mesh: input.ribbon,
                color_property: input.color_property,
                appearance_property: input.appearance_property,
            })?;
        } else {
            if representation_changed {
                self.ribbon
                    .sync_clipping(input.device, input.queue, input.representation)?;
            }
            if self
                .synced
                .is_none_or(|old| old.structure_binding != current.structure_binding)
            {
                self.ribbon
                    .bind(input.device, input.ribbon_layout, input.structure_gpu);
            }
        }
        Ok(())
    }

    fn sync_uniforms(
        &self,
        input: &SlotSync<'_, D>,
        current: &SlotSynced,
        representation_changed: bool,
    ) {
        let resource_changed = self.synced.is_none_or(|old| {
            old.spatial_bounds != current.spatial_bounds
                || old.structure_binding != current.structure_binding
                || old.overlay_binding != current.overlay_binding
        });
        if is_spline(self.kind) || (!representation_changed && !resource_changed) {
            return;
        }
        if let Some(uniforms) = &self.representation_uniforms {
            write_representation_uniforms::<D>(
                input.queue,
                uniforms,
                input.representation,
                input.structure_gpu.bvh_bounds(),
                input.overlay_volume,
            );
        }
    }

    /// Fills the instance scratch for one representation kind.
    fn pack_instances(input: &mut RecordUpload<'_, D>) {
        // Beads aggregate a residue into one sphere, so they take the residue
        // packer and carry no bonds; every other atom-bearing kind packs per
        // atom and keeps its connectivity.
        if input.representation.kind == RepresentationKind::Beads {
            pdviewx_geometry::pack_residue_beads(
                &input.placed.atoms,
                &input.placed.hierarchy,
                input.placed.secondary_structure.values(),
                input.color_property,
                input.representation,
                input.selection,
                input.atoms,
            );
            pdviewx_geometry::build_compaction_map(
                input.atoms,
                input.placed.atoms.len(),
                input.compaction,
            );
            input.bonds.clear();
        } else {
            pdviewx_geometry::pack_atoms_with_properties(
                &input.placed.atoms,
                &input.placed.hierarchy,
                input.placed.secondary_structure.values(),
                pdviewx_geometry::PropertyColumns {
                    color: input.color_property,
                    appearance: input.appearance_property,
                },
                input.representation,
                input.selection,
                input.atoms,
            );
            pdviewx_geometry::build_compaction_map(
                input.atoms,
                input.placed.atoms.len(),
                input.compaction,
            );
            pdviewx_geometry::pack_bonds(
                &input.placed.structure,
                input.representation,
                input.compaction,
                input.bonds,
            );
        }
    }

    fn upload_records(&mut self, input: &mut RecordUpload<'_, D>) -> Result<(), RenderError> {
        Self::pack_instances(input);
        self.atom_count = count(input.atoms.len());
        self.bond_count = count(input.bonds.len());
        upload_grow(
            input.device,
            input.queue,
            "atom instances",
            input.atoms,
            &mut self.atoms,
            &mut self.atoms_capacity,
        )?;
        upload_grow(
            input.device,
            input.queue,
            "bond instances",
            input.bonds,
            &mut self.bonds,
            &mut self.bonds_capacity,
        )?;
        upload_grow(
            input.device,
            input.queue,
            "source-to-compacted atom indices",
            input.compaction,
            &mut self.compaction,
            &mut self.compaction_capacity,
        )?;
        if self.representation_uniforms.is_none() {
            self.representation_uniforms = Some(input.device.create_buffer(&BufferDesc {
                label: "representation uniforms",
                size: std::mem::size_of::<RepresentationUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
        }
        self.sync_surface_resources_from_upload(input)?;
        ensure_indices(
            input.device,
            "visible atom indices",
            self.atom_count,
            &mut self.visible_atoms,
            &mut self.visible_atoms_capacity,
        )?;
        ensure_indices(
            input.device,
            "visible bond indices",
            self.bond_count,
            &mut self.visible_bonds,
            &mut self.visible_bonds_capacity,
        )?;
        ensure_indices(
            input.device,
            "atom visibility cache",
            self.atom_count,
            &mut self.atom_visibility,
            &mut self.atom_visibility_capacity,
        )?;
        self.write_args(input.device, input.queue)?;
        write_counts(
            input.device,
            input.queue,
            self.atom_count,
            self.bond_count,
            &mut self.counts,
        )?;
        self.bind(
            input.device,
            input.layout,
            input.structure_gpu,
            input.surface_field_fallback,
            input.surface_provenance_fallback,
            input.overlay_view,
        );
        self.bind_cull(
            input.device,
            input.cull_layout,
            input.structure_gpu,
            input.frame,
        );
        Ok(())
    }

    fn write_args(&mut self, device: &D, queue: &D::Queue) -> Result<(), RenderError> {
        write_args(device, queue, "atom draw arguments", &mut self.atom_args)?;
        write_args(device, queue, "bond draw arguments", &mut self.bond_args)?;
        write_draw_args(
            device,
            queue,
            "surface draw arguments",
            6,
            u32::from(self.kind == RepresentationKind::Surface && self.atom_count > 0),
            &mut self.surface_args,
        )
    }

    fn sync_surface_resources(
        &mut self,
        input: &SlotSync<'_, D>,
        force_generate: bool,
    ) -> Result<(), RenderError> {
        let (Some(atoms), Some(compaction), Some(uniforms)) =
            (&self.atoms, &self.compaction, &self.representation_uniforms)
        else {
            return Ok(());
        };
        self.surface.sync(&SurfaceSync {
            device: input.device,
            output_layout: input.surface_field_output_layout,
            input_layout: input.surface_field_input_layout,
            erosion_layout: input.surface_field_erosion_layout,
            structure: input.structure_gpu,
            representation: input.representation,
            atoms,
            compaction,
            uniforms,
            atom_count: self.atom_count,
            force_generate,
            overlay_volume: input.overlay_volume,
        })
    }

    fn sync_surface_resources_from_upload(
        &mut self,
        input: &RecordUpload<'_, D>,
    ) -> Result<(), RenderError> {
        let (Some(atoms), Some(compaction), Some(uniforms)) =
            (&self.atoms, &self.compaction, &self.representation_uniforms)
        else {
            return Ok(());
        };
        self.surface.sync(&SurfaceSync {
            device: input.device,
            output_layout: input.surface_field_output_layout,
            input_layout: input.surface_field_input_layout,
            erosion_layout: input.surface_field_erosion_layout,
            structure: input.structure_gpu,
            representation: input.representation,
            atoms,
            compaction,
            uniforms,
            atom_count: self.atom_count,
            force_generate: true,
            overlay_volume: input.overlay_volume,
        })
    }
}

const fn is_spline(kind: RepresentationKind) -> bool {
    matches!(
        kind,
        RepresentationKind::Cartoon
            | RepresentationKind::Trace
            | RepresentationKind::Tube
            | RepresentationKind::Rocket
            | RepresentationKind::Twister
            | RepresentationKind::PaperChain
    )
}

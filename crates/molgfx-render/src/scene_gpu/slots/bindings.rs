//! Persistent bind groups for one molecular representation slot.

use super::GpuSlot;
use crate::scene_gpu::asset_arena::AssetArena;
use crate::scene_gpu::buffers::{arena_range, buffer_entry};
use crate::scene_gpu::structure::GpuStructure;
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, Device};

pub(super) struct CullBinding<'a, D: Device> {
    pub(super) device: &'a D,
    /// The shared packed records this slot draws from.
    pub(super) records: crate::scene_gpu::record_cache::RecordSetRef<'a, D>,
    /// The shared visible set and cull counts for this slot's visibility key.
    pub(super) visibility: crate::scene_gpu::visibility_cache::VisibilitySetRef<'a, D>,
    /// The scene-wide arena holding this slot's argument slots.
    pub(super) arena: Option<&'a D::Buffer>,
    pub(super) atom_layout: &'a D::BindGroupLayout,
    pub(super) bond_layout: &'a D::BindGroupLayout,
    pub(super) visual_layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) frame: &'a D::Buffer,
    pub(super) tiles: &'a D::Buffer,
    pub(super) visual_programs: Option<&'a D::Buffer>,
    pub(super) visual_parameters: Option<&'a D::Buffer>,
    pub(super) visual_properties: Option<&'a D::Buffer>,
}

pub(super) struct RepresentationBinding<'a, D: Device> {
    pub(super) device: &'a D,
    /// The shared packed records this slot draws from.
    pub(super) records: crate::scene_gpu::record_cache::RecordSetRef<'a, D>,
    /// The shared visible set and cull counts for this slot's visibility key.
    pub(super) visibility: crate::scene_gpu::visibility_cache::VisibilitySetRef<'a, D>,
    /// The shared quality hierarchy, absent in a realtime frame.
    pub(super) acceleration:
        Option<&'a crate::scene_gpu::acceleration_cache::SharedAcceleration<D>>,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) quality_layout: &'a D::BindGroupLayout,
    pub(super) structure: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) surface_field_fallback: &'a D::TextureView,
    pub(super) surface_normal_fallback: &'a D::TextureView,
    /// The scene-wide fields this slot's textures are looked up in.
    pub(super) surface_fields: &'a crate::scene_gpu::surface_cache::SurfaceFieldCache<D>,
    pub(super) overlay: &'a D::TextureView,
    pub(super) visual_programs: Option<&'a D::Buffer>,
    pub(super) visual_parameters: Option<&'a D::Buffer>,
    pub(super) visual_properties: Option<&'a D::Buffer>,
}

/// Everything one slot's two groups resolve before either is built.
///
/// Holding the resolved handles rather than re-reading the slot keeps the group
/// construction free of borrows into `self`, so a slot can be borrowed
/// immutably while its group is replaced.
struct Resolved<'a, D: Device> {
    atoms: &'a D::Buffer,
    model: &'a D::Buffer,
    bonds: &'a D::Buffer,
    visible_atoms: &'a D::Buffer,
    visible_bonds: &'a D::Buffer,
    compaction: &'a D::Buffer,
    uniforms: &'a D::Buffer,
    counts: &'a D::Buffer,
    surface_grid: &'a D::TextureView,
    surface_normals: &'a D::TextureView,
    visual: crate::scene_gpu::visual::VisualCullEntries<'a, D>,
}

impl<D: Device> GpuSlot<D> {
    /// Builds the three compute cull groups this slot dispatches.
    ///
    /// A cull group reads the shared records and visible set its slot draws
    /// from, plus the slot's own argument slots in the scene arena, so the
    /// count and index buffers are the shared ones rather than per-slot copies.
    pub(super) fn bind_cull(&mut self, input: &CullBinding<'_, D>) {
        let (Some(atom_args), Some(bond_args), Some(arena), Some(model)) = (
            self.atom_args,
            self.bond_args,
            input.arena,
            input.structure.model.as_ref(),
        ) else {
            return;
        };
        let (Some(visual_programs), Some(visual_parameters), Some(visual_properties)) = (
            input.visual_programs,
            input.visual_parameters,
            input.visual_properties,
        ) else {
            return;
        };
        let Some(visual) =
            self.visual
                .cull_entries(visual_programs, visual_parameters, visual_properties)
        else {
            return;
        };
        let (atoms, bonds, visible_atoms, visible_bonds, counts) = (
            input.records.atoms,
            input.records.bonds,
            input.visibility.visible_atoms,
            input.visibility.visible_bonds,
            input.visibility.counts,
        );
        self.atom_cull_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "atom cull slot",
            layout: input.atom_layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(2, visible_atoms),
                arena_range(4, arena, atom_args),
                arena_range(5, arena, bond_args),
                buffer_entry(6, counts),
                buffer_entry(7, input.frame),
                buffer_entry(8, model),
                input.structure.coords_entry(input.asset_arena, 9),
                buffer_entry(10, input.tiles),
                buffer_entry(14, visual.results),
                buffer_entry(15, visual.config),
            ],
        }));
        self.bond_cull_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "bond cull slot",
            layout: input.bond_layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(1, bonds),
                buffer_entry(2, visible_atoms),
                buffer_entry(3, visible_bonds),
                arena_range(4, arena, atom_args),
                arena_range(5, arena, bond_args),
                buffer_entry(6, counts),
                buffer_entry(7, input.frame),
                buffer_entry(8, model),
                input.structure.coords_entry(input.asset_arena, 9),
                buffer_entry(14, visual.results),
                buffer_entry(15, visual.config),
            ],
        }));
        self.visual_cull_group = Some(input.device.create_bind_group(&BindGroupDesc {
            label: "visual cull slot",
            layout: input.visual_layout,
            entries: &[
                buffer_entry(0, atoms),
                buffer_entry(2, visible_atoms),
                arena_range(4, arena, atom_args),
                buffer_entry(8, model),
                input.structure.coords_entry(input.asset_arena, 9),
                buffer_entry(11, visual.instructions),
                buffer_entry(12, visual.parameters),
                buffer_entry(13, visual.properties),
                buffer_entry(14, visual.results),
                buffer_entry(15, visual.config),
            ],
        }));
    }

    /// The shared field views this slot's group2 binds.
    ///
    /// Kept out of `bind` so that function stays about the groups it builds
    /// rather than about resolving three tables first.
    fn surface_views<'a>(
        &'a self,
        input: &'a RepresentationBinding<'a, D>,
    ) -> (&'a D::TextureView, &'a D::TextureView) {
        (
            self.surface
                .field_binding(input.surface_fields, input.surface_field_fallback),
            self.surface
                .normal_binding(input.surface_fields, input.surface_normal_fallback),
        )
    }

    /// Resolves every handle the two groups bind, or nothing when one is absent.
    fn resolve<'a>(&'a self, input: &'a RepresentationBinding<'a, D>) -> Option<Resolved<'a, D>> {
        let visual = self.visual.cull_entries(
            input.visual_programs?,
            input.visual_parameters?,
            input.visual_properties?,
        )?;
        let (surface_grid, surface_normals) = self.surface_views(input);
        Some(Resolved {
            atoms: input.records.atoms,
            bonds: input.records.bonds,
            visible_atoms: input.visibility.visible_atoms,
            visible_bonds: input.visibility.visible_bonds,
            compaction: input.records.compaction,
            model: input.structure.model.as_ref()?,
            uniforms: self.representation_uniforms.as_ref()?,
            counts: input.visibility.counts,
            surface_grid,
            surface_normals,
            visual,
        })
    }

    /// The colour block, or the representation uniforms as a stand-in.
    ///
    /// A slot always allocates its colour block before binding, so the fallback
    /// exists only to keep the group's binding total constant.
    fn color_entry<'a>(&'a self, uniforms: &'a D::Buffer) -> BindGroupEntry<'a, D> {
        match &self.color_uniforms {
            Some(color) => buffer_entry(18, color),
            None => buffer_entry(18, uniforms),
        }
    }

    /// Builds the raster group: every binding a geometry pass reads.
    fn raster_group<'a>(
        &'a self,
        input: &'a RepresentationBinding<'a, D>,
        resolved: &Resolved<'a, D>,
    ) -> D::BindGroup {
        let Resolved {
            atoms,
            model,
            bonds,
            visible_atoms,
            visible_bonds,
            compaction,
            uniforms,
            counts,
            surface_grid,
            surface_normals,
            visual,
            ..
        } = *resolved;
        input.device.create_bind_group(&BindGroupDesc {
            label: "group2: per-representation",
            layout: input.layout,
            entries: &[
                buffer_entry(0, atoms),
                input.structure.coords_entry(input.asset_arena, 1),
                buffer_entry(2, model),
                buffer_entry(3, bonds),
                buffer_entry(4, visible_atoms),
                buffer_entry(5, visible_bonds),
                input.structure.bvh_nodes_entry(input.asset_arena, 6),
                input.structure.bvh_indices_entry(input.asset_arena, 7),
                buffer_entry(8, compaction),
                buffer_entry(9, uniforms),
                BindGroupEntry::Texture {
                    binding: 10,
                    view: surface_grid,
                },
                BindGroupEntry::Texture {
                    binding: 11,
                    view: surface_normals,
                },
                BindGroupEntry::Texture {
                    binding: 12,
                    view: input.overlay,
                },
                input.structure.previous_coords_entry(input.asset_arena, 13),
                buffer_entry(14, counts),
                buffer_entry(16, visual.results),
                buffer_entry(17, visual.fragment_program),
                self.color_entry(uniforms),
                buffer_entry(19, visual.properties),
                buffer_entry(20, visual.config),
            ],
        })
    }

    /// Builds the quality group: the subset a tracing frame reads, plus the
    /// shared hierarchy it traces against.
    fn quality_group<'a>(
        &'a self,
        input: &'a RepresentationBinding<'a, D>,
        resolved: &Resolved<'a, D>,
    ) -> D::BindGroup {
        let Resolved {
            atoms,
            model,
            compaction,
            uniforms,
            counts,
            visual,
            ..
        } = *resolved;
        input.device.create_bind_group(&BindGroupDesc {
            label: "group2: quality tracing",
            layout: input.quality_layout,
            entries: &[
                buffer_entry(0, atoms),
                input.structure.coords_entry(input.asset_arena, 1),
                buffer_entry(2, model),
                input.structure.bvh_nodes_entry(input.asset_arena, 6),
                input.structure.bvh_indices_entry(input.asset_arena, 7),
                buffer_entry(8, compaction),
                buffer_entry(9, uniforms),
                buffer_entry(14, counts),
                buffer_entry(16, visual.results),
                buffer_entry(17, visual.fragment_program),
                self.color_entry(uniforms),
                buffer_entry(19, visual.properties),
                buffer_entry(20, visual.config),
                match input
                    .acceleration
                    .map(super::super::acceleration_cache::SharedAcceleration::hierarchy)
                    .and_then(|value| value.nodes())
                {
                    Some(nodes) => buffer_entry(21, nodes),
                    None => input.structure.bvh_nodes_entry(input.asset_arena, 21),
                },
            ],
        })
    }

    pub(super) fn bind(&mut self, input: &RepresentationBinding<'_, D>) {
        let Some(resolved) = self.resolve(input) else {
            return;
        };
        let raster = self.raster_group(input, &resolved);
        let quality = self.quality_group(input, &resolved);
        self.group2 = Some(raster);
        self.quality_group = Some(quality);
    }
}

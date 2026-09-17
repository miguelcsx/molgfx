//! Derived synchronization and shading state for a representation slot.

use crate::scene_gpu::slot_types::{
    PresentationState, RecordState, RibbonState, SlotShading, SlotSync, SlotSynced,
};
use molgfx_core::{Representation, RepresentationKind, SurfaceKind, SurfaceStyle, VisualStyle};
use molgfx_gpu::Device;

pub(super) fn synced_state<D: Device>(input: &SlotSync<'_, D>) -> SlotSynced {
    let table = &input.placed.atoms;
    SlotSynced {
        quality: input.quality,
        representation: input.representation_revision,
        presentation: PresentationState::new(input.representation),
        records: RecordState::new(input.representation),
        ribbon: RibbonState::new(input.representation),
        color: table.color().revision().get(),
        flags: table.flags().revision().get(),
        semantic: table.semantic().revision().get(),
        properties: input.property_revisions,
        visual_program: input
            .representation
            .visual
            .as_ref()
            .map_or(0, |style| style.program().fingerprint()),
        visual_parameters: input
            .representation
            .visual
            .as_ref()
            .map_or(0, VisualStyle::parameter_fingerprint),
        visual_properties: input.visual_property_revisions,
        visual_property_binding: input.visual_property_binding_revision,
        visual_program_binding: input.visual_program_binding_revision,
        visual_parameter_binding: input.visual_parameter_binding_revision,
        visual_time: if input.representation.visual.is_some() {
            input.visual_time_revision
        } else {
            0
        },
        secondary_structure: input.placed.secondary_structure.revision().get(),
        bond_topology: if matches!(
            input.representation.kind,
            RepresentationKind::BallAndStick
                | RepresentationKind::Licorice
                | RepresentationKind::Lines
        ) {
            input.placed.bond_topology_revision()
        } else {
            0
        },
        structure_binding: input.structure_gpu.binding_revision,
        coordinates: [
            input.placed.atoms.coords().generation(),
            input.placed.trajectory_revision(),
        ],
        spatial_bounds: [
            input.placed.atoms.coords().generation(),
            input.placed.trajectory_pair_revision(),
        ],
        placement_transform: input
            .placed
            .model_to_world
            .to_cols_array()
            .map(f32::to_bits),
        overlay_binding: input.overlay_binding_revision,
        cull_binding: input.cull_binding_revision,
    }
}

pub(super) fn shading(representation: &Representation) -> SlotShading {
    let fragment_visual = representation
        .visual
        .as_ref()
        .is_some_and(|style| style.program().fragment_instruction_count() != 0);
    SlotShading::default()
        .with_wire(representation.kind == RepresentationKind::Lines)
        .with_clipping(!representation.clipping.planes().is_empty())
        .with_surface_grid(
            matches!(
                representation.params.surface_kind,
                SurfaceKind::SolventExcluded | SurfaceKind::Gaussian
            ) || representation.params.surface_components.is_enabled(),
        )
        .with_surface_atoms(
            representation.kind == RepresentationKind::Surface
                && matches!(
                    representation.params.surface_kind,
                    SurfaceKind::VanDerWaals | SurfaceKind::SolventAccessible
                )
                && !representation.params.surface_components.is_enabled()
                && representation.params.surface_style == SurfaceStyle::Solid
                && !representation.is_translucent(),
        )
        .with_visual(representation.visual.is_some())
        .with_fragment_visual(fragment_visual)
}

pub(super) const fn is_spline(kind: RepresentationKind) -> bool {
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

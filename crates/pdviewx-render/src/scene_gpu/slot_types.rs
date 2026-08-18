//! Borrowed synchronization inputs and stable representation-slot keys.

use super::structure::GpuStructure;
use pdviewx_core::{
    AtomGpu, AtomSelection, BondGpu, ColorScheme, DensityVolume, PlacedStructure, Representation,
    RepresentationHandle, RepresentationKind, RepresentationTarget, StructureHandle,
};
use pdviewx_gpu::Device;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct CullCounts {
    pub(super) atoms: u32,
    pub(super) bonds: u32,
    pub(super) padding: [u32; 2],
}

/// Which specialized pipeline one slot's draw needs.
///
/// The shaders compile a pipeline per combination instead of branching per
/// fragment, so the common path carries no clipping test and no unused
/// tracing code. That choice is made once per representation here and travels
/// with the draw, rather than being rediscovered inside every pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct SlotShading {
    /// The representation carries clip planes and needs the clipped path.
    pub(crate) clipped: bool,
    /// The surface is field-based and traced through the persistent grid
    /// rather than through the analytic union of atom spheres.
    pub(crate) surface_grid: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SlotKey {
    pub(super) structure: StructureHandle,
    pub(super) representation: RepresentationHandle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SlotPlan {
    pub(super) key: SlotKey,
    pub(super) structure_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SlotSynced {
    pub(super) representation: u64,
    pub(super) records: RecordState,
    pub(super) ribbon: RibbonState,
    pub(super) color: u64,
    pub(super) flags: u64,
    pub(super) semantic: u64,
    pub(super) properties: [u64; 2],
    pub(super) secondary_structure: u64,
    pub(super) structure_binding: u64,
    pub(super) coordinates: [u64; 2],
    pub(super) spatial_bounds: [u64; 2],
    pub(super) overlay_binding: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RecordState {
    target: RepresentationTarget,
    kind: u8,
    radius_scale: u32,
    bond_radius: u32,
    opacity: u8,
    color: ColorScheme,
    appearance: Option<pdviewx_core::PropertyAppearance>,
}

impl RecordState {
    pub(super) fn new(representation: &Representation) -> Self {
        Self {
            target: representation.target,
            kind: kind_id(representation.kind),
            radius_scale: representation.params.radius_scale.to_bits(),
            bond_radius: representation.params.bond_radius.to_bits(),
            opacity: representation.material.opacity_unorm8(),
            color: representation.color,
            appearance: representation.appearance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RibbonState {
    target: RepresentationTarget,
    kind: u8,
    width: u32,
    radius: u32,
    radius_mapping: [u32; 5],
    opacity: u8,
    color: ColorScheme,
    appearance: Option<pdviewx_core::PropertyAppearance>,
}

impl RibbonState {
    pub(super) fn new(representation: &Representation) -> Self {
        Self {
            target: representation.target,
            kind: kind_id(representation.kind),
            width: representation.params.ribbon_width.to_bits(),
            radius: representation.params.tube_radius.to_bits(),
            radius_mapping: radius_mapping_id(representation.params.tube_radius_mapping),
            opacity: representation.material.opacity_unorm8(),
            color: representation.color,
            appearance: representation.appearance,
        }
    }
}

fn radius_mapping_id(mapping: pdviewx_core::TubeRadiusMapping) -> [u32; 5] {
    match mapping {
        pdviewx_core::TubeRadiusMapping::Constant => [0; 5],
        pdviewx_core::TubeRadiusMapping::BFactor { domain, radii } => [
            1,
            domain[0].to_bits(),
            domain[1].to_bits(),
            radii[0].to_bits(),
            radii[1].to_bits(),
        ],
    }
}

fn kind_id(kind: RepresentationKind) -> u8 {
    match kind {
        RepresentationKind::Spacefill => 0,
        RepresentationKind::BallAndStick => 1,
        RepresentationKind::Licorice => 2,
        RepresentationKind::Cartoon => 3,
        RepresentationKind::Surface => 4,
        RepresentationKind::Volume => 5,
        RepresentationKind::Points => 6,
        RepresentationKind::Lines => 7,
        RepresentationKind::Trace => 8,
        RepresentationKind::Tube => 9,
        RepresentationKind::Segmentation => 10,
        RepresentationKind::Beads => 11,
        RepresentationKind::Rocket => 12,
        RepresentationKind::Twister => 13,
        RepresentationKind::PaperChain => 14,
        _ => u8::MAX,
    }
}

pub(super) struct SlotSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) ribbon_layout: &'a D::BindGroupLayout,
    pub(super) cull_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_output_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_input_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_erosion_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_fallback: &'a D::TextureView,
    pub(super) surface_provenance_fallback: &'a D::TextureView,
    pub(super) overlay_volume: Option<&'a DensityVolume>,
    pub(super) overlay_view: &'a D::TextureView,
    pub(super) overlay_binding_revision: u64,
    pub(super) frame: &'a D::Buffer,
    pub(super) structure_gpu: &'a GpuStructure<D>,
    pub(super) placed: &'a PlacedStructure,
    pub(super) representation: &'a Representation,
    pub(super) representation_revision: u64,
    pub(super) selection: &'a AtomSelection,
    pub(super) color_property: Option<&'a pdviewx_core::AtomProperty>,
    pub(super) appearance_property: Option<&'a pdviewx_core::AtomProperty>,
    pub(super) property_revisions: [u64; 2],
    pub(super) atoms: &'a mut Vec<AtomGpu>,
    pub(super) bonds: &'a mut Vec<BondGpu>,
    pub(super) compaction: &'a mut Vec<u32>,
    pub(super) ribbon: &'a mut pdviewx_geometry::RibbonMesh,
}

pub(super) struct RecordUpload<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) cull_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_output_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_input_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_erosion_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_fallback: &'a D::TextureView,
    pub(super) surface_provenance_fallback: &'a D::TextureView,
    pub(super) overlay_volume: Option<&'a DensityVolume>,
    pub(super) overlay_view: &'a D::TextureView,
    pub(super) frame: &'a D::Buffer,
    pub(super) structure_gpu: &'a GpuStructure<D>,
    pub(super) placed: &'a PlacedStructure,
    pub(super) representation: &'a Representation,
    pub(super) selection: &'a AtomSelection,
    pub(super) color_property: Option<&'a pdviewx_core::AtomProperty>,
    pub(super) appearance_property: Option<&'a pdviewx_core::AtomProperty>,
    pub(super) atoms: &'a mut Vec<AtomGpu>,
    pub(super) bonds: &'a mut Vec<BondGpu>,
    pub(super) compaction: &'a mut Vec<u32>,
}

pub(crate) struct CullDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) atom_groups: u32,
    pub(crate) bond_groups: u32,
}

impl<D: Device> SlotSync<'_, D> {
    pub(super) fn records(&mut self) -> RecordUpload<'_, D> {
        RecordUpload {
            device: self.device,
            queue: self.queue,
            layout: self.layout,
            cull_layout: self.cull_layout,
            surface_field_output_layout: self.surface_field_output_layout,
            surface_field_input_layout: self.surface_field_input_layout,
            surface_field_erosion_layout: self.surface_field_erosion_layout,
            surface_field_fallback: self.surface_field_fallback,
            surface_provenance_fallback: self.surface_provenance_fallback,
            overlay_volume: self.overlay_volume,
            overlay_view: self.overlay_view,
            frame: self.frame,
            structure_gpu: self.structure_gpu,
            placed: self.placed,
            representation: self.representation,
            selection: self.selection,
            color_property: self.color_property,
            appearance_property: self.appearance_property,
            atoms: &mut *self.atoms,
            bonds: &mut *self.bonds,
            compaction: &mut *self.compaction,
        }
    }
}

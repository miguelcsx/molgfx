//! Borrowed synchronization inputs and stable representation-slot keys.

use crate::scene_gpu::{asset_arena::AssetArena, structure::GpuStructure};
use molgfx_core::{
    AtomSelection, ClipSet, ColorScheme, Material, PlacedStructure, Representation,
    RepresentationHandle, RepresentationKind, RepresentationParams, RepresentationTarget,
    ScalarVolume, Scene, SelectionHandle, StructureHandle, SurfaceScalarOverlay,
};
use molgfx_gpu::Device;

pub(crate) use crate::scene_gpu::draw_family::{
    DRAW_FAMILIES, DrawArgs, DrawFamily, DrawSpecializations, QualityDraw, RibbonDraw,
};

pub(in super::super) use crate::scene_gpu::buffers::CullCounts;

/// Which specialized pipeline one slot's draw needs.
///
/// The shaders compile a pipeline per combination instead of branching per
/// fragment, so the common path carries no clipping test and no unused
/// tracing code. That choice is made once per representation here and travels
/// with the draw, rather than being rediscovered inside every pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct SlotShading(u8);

impl SlotShading {
    const WIRE: u8 = 1 << 0;
    const CLIPPED: u8 = 1 << 1;
    const SURFACE_GRID: u8 = 1 << 2;
    const SURFACE_ATOMS: u8 = 1 << 3;
    const VISUAL: u8 = 1 << 4;
    const FRAGMENT_VISUAL: u8 = 1 << 5;

    pub(crate) const fn with_wire(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::WIRE);
        self
    }

    pub(crate) const fn with_clipping(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::CLIPPED);
        self
    }

    pub(crate) const fn with_surface_grid(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::SURFACE_GRID);
        self
    }

    pub(crate) const fn with_surface_atoms(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::SURFACE_ATOMS);
        self
    }

    pub(crate) const fn with_visual(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::VISUAL);
        self
    }

    pub(crate) const fn with_fragment_visual(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::FRAGMENT_VISUAL);
        self
    }

    const fn flag(enabled: bool, flag: u8) -> u8 {
        if enabled { flag } else { 0 }
    }

    pub(crate) const fn wire(self) -> bool {
        self.0 & Self::WIRE != 0
    }

    pub(crate) const fn clipped(self) -> bool {
        self.0 & Self::CLIPPED != 0
    }

    pub(crate) const fn surface_grid(self) -> bool {
        self.0 & Self::SURFACE_GRID != 0
    }

    pub(crate) const fn surface_atoms(self) -> bool {
        self.0 & Self::SURFACE_ATOMS != 0
    }

    pub(crate) const fn visual(self) -> bool {
        self.0 & Self::VISUAL != 0
    }

    pub(crate) const fn fragment_visual(self) -> bool {
        self.0 & Self::FRAGMENT_VISUAL != 0
    }
}

/// Identity of the records one representation draws from.
///
/// A query-derived selection is identified by the query's own fingerprint, so
/// two representations over the same query share one record set; a hand-built
/// row mask has no query to fingerprint and is identified by its handle.
/// Stable identity of one resident representation slot.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(in super::super) struct SlotKey {
    pub(in super::super) structure: StructureHandle,
    pub(in super::super) representation: RepresentationHandle,
}

/// One frame's plan for a resident slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in super::super) struct SlotPlan {
    pub(in super::super) key: SlotKey,
    pub(in super::super) structure_index: usize,
    pub(in super::super) draw_order: usize,
    pub(in super::super) visible: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct SelectionKey {
    /// The structure the selection's rows index into.
    pub(crate) structure: StructureHandle,
    pub(crate) identity: SelectionIdentity,
    /// The bond topology revision the rows were resolved against.
    pub(crate) topology: u64,
}

/// What a selection is, independent of its handle.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum SelectionIdentity {
    /// A query, named by the fingerprint of its normalized expression.
    Query(u64),
    /// A hand-built row mask. Handles are local to their owning scene.
    Mask { scene: u64, handle: SelectionHandle },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in super::super) struct SlotSynced {
    pub(in super::super) arguments: Option<(u64, u64, u64)>,
    pub(in super::super) visibility: crate::scene_gpu::VisibilityKey,
    pub(in super::super) quality: bool,
    pub(in super::super) detail: crate::scene_gpu::detail::TierDetail,
    /// Which shared record set this slot's draws resolve against.
    pub(in super::super) record_key: crate::scene_gpu::record_cache::RecordKey,
    pub(in super::super) representation: u64,
    pub(in super::super) presentation: PresentationState,
    pub(in super::super) records: RecordState,
    pub(in super::super) ribbon: RibbonState,
    pub(in super::super) color: u64,
    pub(in super::super) flags: u64,
    pub(in super::super) semantic: u64,
    pub(in super::super) properties: [u64; 2],
    /// The selection-scoped overlay and its class column's content revision:
    /// either changing rewrites the colour block and recolours ribbons.
    pub(in super::super) color_overlay: (Option<molgfx_core::ColorOverlay>, u64),
    pub(in super::super) visual_program: u64,
    pub(in super::super) visual_parameters: u64,
    pub(in super::super) visual_properties: [u64; 4],
    pub(in super::super) visual_property_binding: u64,
    pub(in super::super) visual_program_binding: u64,
    pub(in super::super) visual_parameter_binding: u64,
    pub(in super::super) visual_time: u64,
    pub(in super::super) secondary_structure: u64,
    pub(in super::super) bond_topology: u64,
    pub(in super::super) structure_binding: u64,
    pub(in super::super) coordinates: [u64; 2],
    pub(in super::super) spatial_bounds: [u64; 2],
    pub(in super::super) placement_transform: [u32; 16],
    pub(in super::super) overlay_binding: u64,
    pub(in super::super) cull_binding: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in super::super) struct PresentationState {
    material: Material,
    params: RepresentationParams,
    clipping: ClipSet,
    surface_scalar: Option<SurfaceScalarOverlay>,
    /// How atoms are coloured, and the physical mapping that drives opacity.
    ///
    /// Both are applied on the GPU from the representation uniform, so they
    /// belong here: a scheme or appearance change rewrites one uniform block
    /// and leaves every packed record untouched.
    color: ColorScheme,
    appearance: Option<molgfx_core::PropertyAppearance>,
}

impl PresentationState {
    pub(in super::super) const fn new(representation: &Representation) -> Self {
        Self {
            material: representation.material,
            params: representation.params,
            clipping: representation.clipping,
            surface_scalar: representation.surface_scalar,
            color: representation.color,
            appearance: representation.appearance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RecordState {
    target: RepresentationTarget,
    kind: u8,
    radius_scale: u32,
    bond_radius: u32,
    distance_cutoff: u32,
    surface: [u32; 4],
}

impl RecordState {
    pub(in super::super) fn new(representation: &Representation) -> Self {
        Self {
            target: representation.target,
            kind: kind_id(representation.kind),
            radius_scale: representation.params.radius_scale.to_bits(),
            bond_radius: representation.params.bond_radius.to_bits(),
            distance_cutoff: representation.params.distance_cutoff.to_bits(),
            surface: [
                representation.params.probe_radius.to_bits(),
                representation.params.isolevel.to_bits(),
                representation.params.surface_kind as u32,
                representation.params.surface_style as u32,
            ],
        }
    }

    pub(in super::super) const fn target(&self) -> RepresentationTarget {
        self.target
    }

    pub(in super::super) const fn kind(&self) -> u8 {
        self.kind
    }

    /// The radius scalar as stored, so a geometry projection can order it.
    pub(in super::super) const fn radius_scale_bits(&self) -> u32 {
        self.radius_scale
    }

    pub(in super::super) const fn radius_scale(&self) -> u32 {
        self.radius_scale
    }

    pub(in super::super) const fn distance_cutoff(&self) -> u32 {
        self.distance_cutoff
    }

    pub(in super::super) const fn bond_radius(&self) -> u32 {
        self.bond_radius
    }

    pub(in super::super) const fn surface(&self) -> [u32; 4] {
        self.surface
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in super::super) struct RibbonState {
    surface_geometry: u64,
    surface_wire: bool,
    cutoff: u32,
    max_ring_size: u8,
    target: RepresentationTarget,
    kind: u8,
    width: u32,
    aspect_ratio: u32,
    helix_radius: u32,
    arrow_factor: u32,
    direction_wedges: bool,
    gaps: molgfx_core::GapStyle,
    helix_profile: molgfx_core::CartoonProfile,
    nucleic_profile: molgfx_core::CartoonProfile,
    radius: u32,
    color: ColorScheme,
    appearance: Option<molgfx_core::PropertyAppearance>,
    steps: u8,
}

impl RibbonState {
    pub(in super::super) fn new(representation: &Representation, steps: u8) -> Self {
        Self {
            steps,
            surface_geometry: representation
                .molecular_surface
                .as_ref()
                .map_or(0, molgfx_core::MolecularSurfaceGeometry::cache_identity),
            surface_wire: representation.kind == RepresentationKind::MolecularSurface
                && representation.params.surface_style == molgfx_core::SurfaceStyle::Mesh,
            target: representation.target,
            kind: kind_id(representation.kind),
            cutoff: representation.params.distance_cutoff.to_bits(),
            max_ring_size: representation.params.max_ring_size,
            width: representation.params.ribbon_width.to_bits(),
            aspect_ratio: representation.params.cartoon_aspect_ratio.to_bits(),
            helix_radius: representation.params.cartoon_helix_radius.to_bits(),
            arrow_factor: representation.params.cartoon_arrow_factor.to_bits(),
            direction_wedges: representation.params.cartoon_direction_wedges,
            gaps: representation.params.gaps,
            helix_profile: representation.params.cartoon_helix_profile,
            nucleic_profile: representation.params.cartoon_nucleic_profile,
            radius: representation.params.tube_radius.to_bits(),
            color: representation.color,
            appearance: representation.appearance,
        }
    }
}

/// Elements above which the cull shader switches to its wide index path.
pub(crate) const LOD_ATOM_THRESHOLD: u32 = 131_072;

/// The cull mode that drops only atoms an opaque nearer atom hides.
pub(crate) const OCCLUSION_CULL_MODE: u32 = 3;

/// Largest instance count the fast point-index path can address.
pub(crate) const FAST_POINT_INDEX_LIMIT: u32 = 1 << 24;

/// One slot's culling policy: the packed records it reads plus the scalars the
/// cull shaders vary on. Appearance is deliberately absent.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct CullPolicy {
    pub(crate) records: crate::scene_gpu::record_cache::RecordKey,
    pub(crate) kind: RepresentationKind,
    pub(crate) visual_enabled: bool,
    pub(crate) lod_enabled: bool,
    pub(crate) opaque: bool,
    /// Model-space length past which a stretched bond is culled.
    pub(crate) bond_break_length: f32,
    /// Vertices per atom in this draw, independent of the packed records.
    pub(crate) atom_vertices: u32,
}

impl CullPolicy {
    pub(crate) fn new(
        records: crate::scene_gpu::RecordKey,
        representation: &Representation,
        placed: &PlacedStructure,
        lod_enabled: bool,
    ) -> Self {
        Self {
            records,
            kind: representation.kind,
            visual_enabled: representation.visual.is_some(),
            lod_enabled,
            opaque: !representation.is_translucent(),
            atom_vertices: atom_vertex_count(representation),
            bond_break_length: if representation.kind == RepresentationKind::DynamicBonds {
                representation.params.distance_cutoff
            } else {
                placed.bond_break_length()
            },
        }
    }
    pub(crate) fn visibility_key(self, atoms: u32, bonds: u32) -> crate::scene_gpu::VisibilityKey {
        crate::scene_gpu::VisibilityKey {
            records: self.records,
            lod_mode: lod_mode(atoms, bonds, self.kind, self.lod_enabled, self.opaque),
            visual_enabled: self.visual_enabled,
            bond_break_length: self.bond_break_length.to_bits(),
        }
    }
}

pub(crate) fn atom_vertex_count(representation: &Representation) -> u32 {
    if matches!(
        representation.kind,
        RepresentationKind::Dots | RepresentationKind::Solvent
    ) {
        6 * u32::from(representation.params.point_samples.clamp(1, 4096))
    } else {
        6
    }
}

/// The cull mode the shader reads for one packed record set, shared by
/// representations whose counts and drawn form agree. Modes 1 and 2 are
/// approximate; mode 3 only drops atoms an opaque nearer atom hides.
#[must_use]
pub(crate) const fn lod_mode(
    atoms: u32,
    bonds: u32,
    kind: RepresentationKind,
    enabled: bool,
    opaque: bool,
) -> u32 {
    if atoms < LOD_ATOM_THRESHOLD {
        return 0;
    }
    if !enabled {
        let lone_spheres = opaque && bonds == 0 && matches!(kind, RepresentationKind::Spacefill);
        return if lone_spheres { OCCLUSION_CULL_MODE } else { 0 };
    }
    if matches!(
        kind,
        RepresentationKind::Cartoon | RepresentationKind::Surface
    ) {
        0
    } else if matches!(kind, RepresentationKind::Points)
        && bonds == 0
        && atoms < FAST_POINT_INDEX_LIMIT
    {
        2
    } else {
        1
    }
}

fn kind_id(kind: RepresentationKind) -> u8 {
    match kind {
        RepresentationKind::Spacefill => 0,
        RepresentationKind::BallAndStick => 1,
        RepresentationKind::Licorice => 2,
        RepresentationKind::Bonds => 18,
        RepresentationKind::DynamicBonds => 21,
        RepresentationKind::NewRibbons => 19,
        RepresentationKind::Ribbons => 23,
        RepresentationKind::Polyhedra => 20,
        RepresentationKind::Cartoon => 3,
        RepresentationKind::Surface => 4,
        RepresentationKind::MolecularSurface => 24,
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
        RepresentationKind::Dots => 15,
        RepresentationKind::Solvent => 22,
        RepresentationKind::Bases => 16,
        RepresentationKind::BasePairs => 17,
    }
}

pub(in super::super) struct SlotSync<'a, D: Device> {
    pub(in super::super) device: &'a D,
    pub(in super::super) queue: &'a D::Queue,
    /// The shared packed records this slot draws from.
    pub(in super::super) records: crate::scene_gpu::record_cache::RecordSetRef<'a, D>,
    /// The scene-wide surface fields this slot's textures come from.
    pub(in super::super) surface_fields: &'a crate::scene_gpu::surface_cache::SurfaceFieldCache<D>,
    /// The shared visible set and cull counts for this slot's visibility key.
    pub(in super::super) visibility: crate::scene_gpu::visibility_cache::VisibilitySetRef<'a, D>,
    /// Byte offsets of this slot's argument slots in the scene arena.
    pub(in super::super) args: Option<(u64, u64, u64)>,
    /// The scene arena the cull groups bind as ranged slices.
    pub(in super::super) draw_arena: Option<&'a D::Buffer>,
    /// The shared quality hierarchy, or `None` in a realtime frame.
    pub(in super::super) acceleration_draw:
        Option<&'a crate::scene_gpu::acceleration_cache::SharedAcceleration<D>>,
    pub(in super::super) layout: &'a D::BindGroupLayout,
    pub(in super::super) quality_layout: &'a D::BindGroupLayout,
    pub(in super::super) ray_query_layout: Option<&'a D::BindGroupLayout>,
    pub(in super::super) ribbon_layout: &'a D::BindGroupLayout,
    pub(in super::super) helix_fit_layout: &'a D::BindGroupLayout,
    pub(in super::super) atom_cull_layout: &'a D::BindGroupLayout,
    pub(in super::super) bond_cull_layout: &'a D::BindGroupLayout,
    pub(in super::super) visual_cull_layout: &'a D::BindGroupLayout,
    pub(in super::super) surface_field_fallback: &'a D::TextureView,
    pub(in super::super) overlay_volume: Option<&'a ScalarVolume>,
    pub(in super::super) overlay_view: &'a D::TextureView,
    pub(in super::super) overlay_binding_revision: u64,
    pub(in super::super) quality: bool,
    pub(in super::super) detail: crate::scene_gpu::detail::TierDetail,
    pub(in super::super) frame: &'a D::Buffer,
    pub(in super::super) cull_tiles: &'a D::Buffer,
    pub(in super::super) cull_binding_revision: u64,
    pub(in super::super) structure_gpu: &'a GpuStructure<D>,
    pub(in super::super) asset_arena: &'a AssetArena<D>,
    pub(in super::super) asset_identity: crate::scene_gpu::asset::GpuAssetIdentity,
    /// The scene, for canonical query identity.
    pub(in super::super) scene: &'a Scene,
    pub(in super::super) placed: &'a PlacedStructure,
    pub(in super::super) representation: &'a Representation,
    pub(in super::super) representation_revision: u64,
    pub(in super::super) selection: &'a AtomSelection,
    /// Handle of `selection`, needed to key the shared packed records.
    pub(in super::super) selection_handle: molgfx_core::SelectionHandle,
    pub(in super::super) color: molgfx_geometry::ColorContext<'a>,
    pub(in super::super) property_revisions: [u64; 2],
    pub(in super::super) visual_property_buffer: Option<&'a D::Buffer>,
    pub(in super::super) visual_program_buffer: Option<&'a D::Buffer>,
    pub(in super::super) visual_program_offset: u32,
    pub(in super::super) visual_program_binding_revision: u64,
    pub(in super::super) visual_parameter_buffer: Option<&'a D::Buffer>,
    pub(in super::super) visual_parameter_offset: u32,
    pub(in super::super) visual_parameter_binding_revision: u64,
    pub(in super::super) visual_property_offsets: [u32; 4],
    pub(in super::super) visual_attribute_layouts: [u32; 4],
    pub(in super::super) visual_state_offset: u32,
    pub(in super::super) visual_property_binding_revision: u64,
    pub(in super::super) visual_property_revisions: [u64; 4],
    pub(in super::super) visual_time_seconds: f32,
    pub(in super::super) visual_time_revision: u64,
    pub(in super::super) ribbon: &'a mut molgfx_geometry::RibbonMesh,
}

pub(crate) use super::slot_culling::{CullDispatch, CullModes};

#[cfg(test)]
#[path = "slot_types_tests.rs"]
mod tests;

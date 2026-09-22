//! Borrowed synchronization inputs and stable representation-slot keys.

use super::asset_arena::AssetArena;
use super::structure::GpuStructure;
use molgfx_core::{
    AtomSelection, ClipSet, ColorScheme, Material, PlacedStructure, Representation,
    RepresentationHandle, RepresentationKind, RepresentationParams, RepresentationTarget,
    ScalarVolume, Scene, SelectionHandle, StructureHandle, SurfaceScalarOverlay,
};
use molgfx_gpu::Device;

pub(crate) use super::draw_family::{
    DRAW_FAMILIES, DrawArgs, DrawFamily, DrawSpecializations, QualityDraw, RibbonDraw,
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct CullCounts {
    pub(super) atoms: u32,
    pub(super) bonds: u32,
    pub(super) lod_enabled: u32,
    pub(super) padding: u32,
    /// Model-space length past which a stretched bond is culled; zero disables.
    pub(super) bond_break_length: f32,
    /// One when a typed visual result table should affect culling/shading.
    pub(super) visual_enabled: u32,
    pub(super) atom_bvh_nodes: u32,
    pub(super) atom_bvh_indices: u32,
    pub(super) bond_bvh_nodes: u32,
    pub(super) bond_bvh_indices: u32,
}

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
pub(super) struct SlotKey {
    pub(super) structure: StructureHandle,
    pub(super) representation: RepresentationHandle,
}

/// One frame's plan for a resident slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SlotPlan {
    pub(super) key: SlotKey,
    pub(super) structure_index: usize,
    pub(super) draw_order: usize,
    pub(super) visible: bool,
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
    /// A hand-built row mask, named by the handle that owns it.
    Mask(SelectionHandle),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SlotSynced {
    pub(super) quality: bool,
    /// Which shared record set this slot's draws resolve against.
    pub(super) record_key: super::record_cache::RecordKey,
    pub(super) representation: u64,
    pub(super) presentation: PresentationState,
    pub(super) records: RecordState,
    pub(super) ribbon: RibbonState,
    pub(super) color: u64,
    pub(super) flags: u64,
    pub(super) semantic: u64,
    pub(super) properties: [u64; 2],
    pub(super) visual_program: u64,
    pub(super) visual_parameters: u64,
    pub(super) visual_properties: [u64; 4],
    pub(super) visual_property_binding: u64,
    pub(super) visual_program_binding: u64,
    pub(super) visual_parameter_binding: u64,
    pub(super) visual_time: u64,
    pub(super) secondary_structure: u64,
    pub(super) bond_topology: u64,
    pub(super) structure_binding: u64,
    pub(super) coordinates: [u64; 2],
    pub(super) spatial_bounds: [u64; 2],
    pub(super) placement_transform: [u32; 16],
    pub(super) overlay_binding: u64,
    pub(super) cull_binding: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PresentationState {
    material: Material,
    params: RepresentationParams,
    clipping: ClipSet,
    surface_scalar: Option<SurfaceScalarOverlay>,
    /// How atoms are coloured, and the scientific mapping that drives opacity.
    ///
    /// Both are applied on the GPU from the representation uniform, so they
    /// belong here: a scheme or appearance change rewrites one uniform block
    /// and leaves every packed record untouched.
    color: ColorScheme,
    appearance: Option<molgfx_core::PropertyAppearance>,
}

impl PresentationState {
    pub(super) const fn new(representation: &Representation) -> Self {
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
    surface: [u32; 4],
}

impl RecordState {
    pub(super) fn new(representation: &Representation) -> Self {
        Self {
            target: representation.target,
            kind: kind_id(representation.kind),
            radius_scale: representation.params.radius_scale.to_bits(),
            bond_radius: representation.params.bond_radius.to_bits(),
            surface: [
                representation.params.probe_radius.to_bits(),
                representation.params.isolevel.to_bits(),
                representation.params.surface_kind as u32,
                representation.params.surface_style as u32,
            ],
        }
    }

    pub(super) const fn target(&self) -> RepresentationTarget {
        self.target
    }

    pub(super) const fn kind(&self) -> u8 {
        self.kind
    }

    /// The radius scalar as stored, so a geometry projection can order it.
    pub(super) const fn radius_scale_bits(&self) -> u32 {
        self.radius_scale
    }

    pub(super) const fn radius_scale(&self) -> u32 {
        self.radius_scale
    }

    pub(super) const fn bond_radius(&self) -> u32 {
        self.bond_radius
    }

    pub(super) const fn surface(&self) -> [u32; 4] {
        self.surface
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RibbonState {
    target: RepresentationTarget,
    kind: u8,
    width: u32,
    radius: u32,
    color: ColorScheme,
    appearance: Option<molgfx_core::PropertyAppearance>,
}

impl RibbonState {
    pub(super) fn new(representation: &Representation) -> Self {
        Self {
            target: representation.target,
            kind: kind_id(representation.kind),
            width: representation.params.ribbon_width.to_bits(),
            radius: representation.params.tube_radius.to_bits(),
            color: representation.color,
            appearance: representation.appearance,
        }
    }
}

/// Elements above which the cull shader switches to its wide index path.
pub(crate) const LOD_ATOM_THRESHOLD: u32 = 131_072;

/// Largest instance count the fast point-index path can address.
pub(crate) const FAST_POINT_INDEX_LIMIT: u32 = 1 << 24;

/// One slot's culling policy: the packed records it reads plus the scalars the
/// cull shaders vary on. Appearance is deliberately absent.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct CullPolicy {
    pub(crate) records: super::record_cache::RecordKey,
    pub(crate) kind: RepresentationKind,
    pub(crate) visual_enabled: bool,
    /// Model-space length past which a stretched bond is culled.
    pub(crate) bond_break_length: f32,
}

/// The level-of-detail mode the cull shader reads for one packed record set.
///
/// Two representations sharing records share this, because it depends only on
/// the counts and the drawn form.
#[must_use]
pub(crate) const fn lod_mode(atoms: u32, bonds: u32, kind: RepresentationKind) -> u32 {
    if atoms < LOD_ATOM_THRESHOLD {
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
    }
}

pub(super) struct SlotSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    /// The shared packed records this slot draws from.
    pub(super) records: super::record_cache::RecordSetRef<'a, D>,
    /// The scene-wide surface fields this slot's textures come from.
    pub(super) surface_fields: &'a super::surface_cache::SurfaceFieldCache<D>,
    /// The shared visible set and cull counts for this slot's visibility key.
    pub(super) visibility: super::visibility_cache::VisibilitySetRef<'a, D>,
    /// Byte offsets of this slot's argument slots in the scene arena.
    pub(super) args: Option<(u64, u64, u64)>,
    /// The scene arena the cull groups bind as ranged slices.
    pub(super) draw_arena: Option<&'a D::Buffer>,
    /// The shared quality hierarchy, or `None` in a realtime frame.
    pub(super) acceleration_draw: Option<&'a super::acceleration_cache::SharedAcceleration<D>>,
    pub(super) layout: &'a D::BindGroupLayout,
    pub(super) quality_layout: &'a D::BindGroupLayout,
    pub(super) ray_query_layout: Option<&'a D::BindGroupLayout>,
    pub(super) ribbon_layout: &'a D::BindGroupLayout,
    pub(super) atom_cull_layout: &'a D::BindGroupLayout,
    pub(super) bond_cull_layout: &'a D::BindGroupLayout,
    pub(super) visual_cull_layout: &'a D::BindGroupLayout,
    pub(super) surface_field_fallback: &'a D::TextureView,
    pub(super) surface_normal_fallback: &'a D::TextureView,
    pub(super) overlay_volume: Option<&'a ScalarVolume>,
    pub(super) overlay_view: &'a D::TextureView,
    pub(super) overlay_binding_revision: u64,
    pub(super) quality: bool,
    pub(super) frame: &'a D::Buffer,
    pub(super) cull_tiles: &'a D::Buffer,
    pub(super) cull_binding_revision: u64,
    pub(super) structure_gpu: &'a GpuStructure<D>,
    pub(super) asset_arena: &'a AssetArena<D>,
    pub(super) asset_identity: super::asset::GpuAssetIdentity,
    /// The scene, for canonical query identity.
    pub(super) scene: &'a Scene,
    pub(super) placed: &'a PlacedStructure,
    pub(super) representation: &'a Representation,
    pub(super) representation_revision: u64,
    pub(super) selection: &'a AtomSelection,
    /// Handle of `selection`, needed to key the shared packed records.
    pub(super) selection_handle: molgfx_core::SelectionHandle,
    pub(super) color_property: Option<&'a molgfx_core::AtomProperty>,
    pub(super) appearance_property: Option<&'a molgfx_core::AtomProperty>,
    pub(super) property_revisions: [u64; 2],
    pub(super) visual_property_buffer: Option<&'a D::Buffer>,
    pub(super) visual_program_buffer: Option<&'a D::Buffer>,
    pub(super) visual_program_offset: u32,
    pub(super) visual_program_binding_revision: u64,
    pub(super) visual_parameter_buffer: Option<&'a D::Buffer>,
    pub(super) visual_parameter_offset: u32,
    pub(super) visual_parameter_binding_revision: u64,
    pub(super) visual_property_offsets: [u32; 4],
    pub(super) visual_attribute_layouts: [u32; 4],
    pub(super) visual_state_offset: u32,
    pub(super) visual_property_binding_revision: u64,
    pub(super) visual_property_revisions: [u64; 4],
    pub(super) visual_time_seconds: f32,
    pub(super) visual_time_revision: u64,
    pub(super) ribbon: &'a mut molgfx_geometry::RibbonMesh,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct CullModes(u8);

impl CullModes {
    const LOD: u8 = 1 << 0;
    const FAST_POINTS: u8 = 1 << 1;
    const DIRECT_BONDS: u8 = 1 << 2;
    const CULL_VISUAL: u8 = 1 << 3;
    const SHADING_VISUAL: u8 = 1 << 4;
    const SHADING_ALL: u8 = 1 << 5;

    pub(crate) const fn with_lod(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::LOD);
        self
    }

    pub(crate) const fn with_fast_points(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::FAST_POINTS);
        self
    }

    pub(crate) const fn with_direct_bonds(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::DIRECT_BONDS);
        self
    }

    pub(crate) const fn with_cull_visual(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::CULL_VISUAL);
        self
    }

    pub(crate) const fn with_shading_visual(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::SHADING_VISUAL);
        self
    }

    pub(crate) const fn with_shading_all(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::SHADING_ALL);
        self
    }

    const fn flag(enabled: bool, flag: u8) -> u8 {
        if enabled { flag } else { 0 }
    }

    pub(crate) const fn lod(self) -> bool {
        self.0 & Self::LOD != 0
    }

    pub(crate) const fn fast_points(self) -> bool {
        self.0 & Self::FAST_POINTS != 0
    }

    pub(crate) const fn direct_bonds(self) -> bool {
        self.0 & Self::DIRECT_BONDS != 0
    }

    pub(crate) const fn cull_visual(self) -> bool {
        self.0 & Self::CULL_VISUAL != 0
    }

    pub(crate) const fn shading_visual(self) -> bool {
        self.0 & Self::SHADING_VISUAL != 0
    }

    pub(crate) const fn shading_all(self) -> bool {
        self.0 & Self::SHADING_ALL != 0
    }
}

pub(crate) struct CullDispatch<'a, D: Device> {
    /// The visibility key this dispatch computes, when the slot has one.
    pub(crate) key: Option<super::visibility_cache::VisibilityKey>,
    pub(crate) atom_group: &'a D::BindGroup,
    pub(crate) bond_group: &'a D::BindGroup,
    pub(crate) visual_group: &'a D::BindGroup,
    pub(crate) atom_groups: [u32; 2],
    pub(crate) bin_groups: [u32; 2],
    pub(crate) tile_groups: [u32; 2],
    pub(crate) modes: CullModes,
    pub(crate) bond_groups: [u32; 2],
    pub(crate) visual_groups: [u32; 2],
}

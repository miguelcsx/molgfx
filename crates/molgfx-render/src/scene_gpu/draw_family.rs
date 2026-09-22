//! Which generated pipeline a draw may use.
//!
//! A family is one fragment entry of one pass: it names the exact unit a
//! specialized sibling replaces, so the sphere impostor and its clipped twin are
//! separate families even though one pass records both. A slot resolves one key
//! per family before any pass records, and each draw carries the pipeline its own
//! family resolved to.

use super::slot_types::SlotShading;
use crate::engine::pipeline_cache::SpecializationKey;
use molgfx_gpu::Device;

/// Which generated pipeline a draw may use, named by the family that draws it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DrawFamily {
    /// Unclipped sphere impostors.
    Sphere,
    /// Sphere impostors with a clip-plane test.
    SphereClipped,
    /// Analytic bond capsules.
    Bond,
    /// Bond lines.
    BondWire,
    /// Pixel-stable atom points.
    Point,
    /// Transport-framed cartoon ribbons.
    Cartoon,
    /// The analytic union of atom spheres.
    SurfaceUnion,
    /// The marched persistent field.
    SurfaceGrid,
    /// Depth-only analytic atom shadows.
    ShadowSphere,
    /// Depth-only analytic bond shadows.
    ShadowBond,
    /// Depth-only ribbon shadows.
    ShadowRibbon,
    /// Progressive analytic occlusion.
    AmbientOcclusion,
    /// Progressive occlusion traversed by hardware ray queries.
    AmbientOcclusionRayQuery,
}

/// Number of families a slot resolves, sizing its key table.
pub(crate) const DRAW_FAMILIES: usize = 13;

impl DrawFamily {
    /// Every family, in slot-key table order.
    pub(crate) const ALL: [Self; DRAW_FAMILIES] = [
        Self::Sphere,
        Self::SphereClipped,
        Self::Bond,
        Self::BondWire,
        Self::Point,
        Self::Cartoon,
        Self::SurfaceUnion,
        Self::SurfaceGrid,
        Self::ShadowSphere,
        Self::ShadowBond,
        Self::ShadowRibbon,
        Self::AmbientOcclusion,
        Self::AmbientOcclusionRayQuery,
    ];

    /// This family's index in the slot key table.
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    /// How many families a slot's key table holds.
    pub(crate) const COUNT: usize = DRAW_FAMILIES;

    /// The stable family name the cache keys on.
    ///
    /// One name per distinct fragment unit, so two variants of one pass can
    /// never share a cached pipeline and be handed each other's code.
    #[must_use]
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Sphere => "sphere",
            Self::SphereClipped => "sphere_clipped",
            Self::Bond => "bond",
            Self::BondWire => "bond_wire",
            Self::Point => "point",
            Self::Cartoon => "cartoon",
            Self::SurfaceUnion => "surface",
            Self::SurfaceGrid => "surface_grid",
            Self::ShadowSphere => "shadow_sphere",
            Self::ShadowBond => "shadow_bond",
            Self::ShadowRibbon => "shadow_ribbon",
            Self::AmbientOcclusion => "ambient_occlusion",
            Self::AmbientOcclusionRayQuery => "ambient_occlusion_ray_query",
        }
    }
}

/// One slot's settled key per drawable family.
///
/// A slot names the family it draws through but cannot resolve a pipeline: the
/// cache belongs to the scene, so the slot carries the key and the scene-level
/// iterator turns it into a pipeline. Splitting it there is what keeps the
/// settle pass the only thing that can compile.
pub(crate) type DrawSpecializations = [Option<SpecializationKey>; DRAW_FAMILIES];

/// One draw's live resources: its bind group, the argument buffer, the byte
/// offset of its slot in that buffer, the shading it needs, and the generated
/// pipeline this slot's family resolved to, when it has one.
pub(crate) type DrawArgs<'a, D> = (
    &'a <D as Device>::BindGroup,
    u64,
    SlotShading,
    Option<&'a <D as Device>::Pipeline>,
);

/// One ribbon draw: its bind group, its argument buffer, the shading it needs
/// and the generated pipeline its family resolved to.
pub(crate) type RibbonDraw<'a, D> = (
    &'a <D as Device>::BindGroup,
    &'a <D as Device>::Buffer,
    SlotShading,
    Option<&'a <D as Device>::Pipeline>,
);

pub(crate) type QualityDraw<'a, D> = (
    &'a <D as Device>::BindGroup,
    Option<&'a <D as Device>::BindGroup>,
    SlotShading,
    Option<&'a <D as Device>::Pipeline>,
);

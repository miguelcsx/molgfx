//! Per-representation surface state: which shared field this slot shades.
//!
//! The textures, the normals, the component working set and every bind group
//! over them live in [`super::surface_cache::SurfaceFieldCache`], keyed by
//! geometry and sampling policy. Two surfaces that differ only in colour,
//! opacity, material, pattern or visual style therefore resolve to one field
//! and generate it once.
//!
//! What remains here is genuinely per-representation: the key this slot
//! resolved, and whether it shades a surface at all. Keys are resolved for
//! every slot before the cache is populated, because populating it needs the
//! cache mutably while binding needs it immutably — resolving first keeps those
//! two borrows in separate passes rather than fighting inside one.

use super::record_cache::RecordGeometry;
use super::surface_cache::SurfaceFieldCache;
use super::surface_field::{FieldSampling, SharedField, SurfaceFieldKey};
use super::uniforms::RepresentationUniforms;
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;

#[derive(Debug)]
pub(super) struct SurfaceSlot {
    /// The field this slot shades, absent when the slot shades no surface.
    key: Option<SurfaceFieldKey>,
}

impl SurfaceSlot {
    pub(super) const fn new() -> Self {
        Self { key: None }
    }

    /// The field key this slot shades, and the key the frame must retain.
    #[must_use]
    pub(super) const fn key(&self) -> Option<SurfaceFieldKey> {
        self.key
    }

    /// Resolves this slot's field key from the uniforms it will be drawn with.
    ///
    /// The key covers geometry and sampling policy only: the record key stands
    /// in for the sampled geometry, and the uniform carries the boundary kind,
    /// probe, radius scale, iso-level and grid. Colour, opacity, material,
    /// pattern and visual style are all absent, which is exactly what lets two
    /// differently-styled surfaces share one field. A slot that shades no
    /// surface resolves a key of `None`, and a slot whose key is unchanged is
    /// left alone.
    pub(super) fn resolve_key(
        &mut self,
        uniforms: &RepresentationUniforms,
        representation: &molgfx_core::Representation,
        geometry: RecordGeometry,
        atom_count: u32,
    ) {
        let shades_surface = representation.kind == RepresentationKind::Surface
            && (representation.params.surface_kind != molgfx_core::SurfaceKind::VanDerWaals
                || representation.params.surface_components.is_enabled())
            && atom_count > 0;
        self.key = shades_surface.then(|| key_of(uniforms, representation, geometry));
    }

    /// The boundary this slot shades, or the scene fallback when it has none.
    ///
    /// Looked up rather than stored, so the slot never holds a second handle to
    /// a texture the cache owns and the two stay independently borrowable.
    pub(super) fn field_binding<'a, D: Device>(
        &self,
        fields: &'a SurfaceFieldCache<D>,
        fallback: &'a D::TextureView,
    ) -> &'a D::TextureView {
        match self.key.and_then(|key| fields.get(key)) {
            Some(field) => field.shading_field(),
            None => fallback,
        }
    }

    /// The normals of [`Self::field_binding`], or the scene fallback.
    pub(super) fn normal_binding<'a, D: Device>(
        &self,
        fields: &'a SurfaceFieldCache<D>,
        fallback: &'a D::TextureView,
    ) -> &'a D::TextureView {
        match self.key.and_then(|key| fields.get(key)) {
            Some(field) => &field.normals.view,
            None => fallback,
        }
    }
}

/// The field key one representation and its uniforms describe.
#[must_use]
pub(super) fn key_of(
    uniforms: &RepresentationUniforms,
    representation: &molgfx_core::Representation,
    geometry: RecordGeometry,
) -> SurfaceFieldKey {
    SurfaceFieldKey::new(
        representation,
        geometry,
        FieldSampling {
            probe: uniforms.surface[0],
            sigma: uniforms.surface[2],
            radius_scale: uniforms.visual[3],
            isolevel: uniforms.surface[1],
            grid_min: [
                uniforms.grid_min[0],
                uniforms.grid_min[1],
                uniforms.grid_min[2],
            ],
            grid_cell: uniforms.grid_cell[0],
            grid_size: uniforms.grid_size,
        },
    )
}

/// Records one shared field's generation passes, clearing its debt.
///
/// The debt lives on the shared field, so the first surface to reach it
/// generates and every later surface over the same key finds it clean.
pub(super) fn record_generation<D: Device>(
    encoder: &mut D::CommandEncoder,
    pass: &crate::passes::SurfaceFieldPass<D>,
    components: &crate::passes::SurfaceComponentPass<D>,
    field: &mut SharedField<D>,
) {
    if !field.pending {
        return;
    }
    pass.record_generate(
        encoder,
        &field.output,
        &field.input,
        field.dimensions,
        field.gaussian,
    );
    if let Some(erosion) = &field.erosion {
        pass.record_erode(encoder, erosion, &field.input, field.dimensions);
    }
    if let Some(group) = field
        .components
        .as_ref()
        .and_then(super::surface_components::SurfaceComponents::group)
    {
        components.record(encoder, group, field.dimensions);
    }
    pass.record_normals(
        encoder,
        &field.normal_output,
        &field.input,
        field.dimensions,
    );
    field.pending = false;
}

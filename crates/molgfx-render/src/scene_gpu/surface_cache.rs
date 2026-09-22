//! The cache that makes two surfaces share one field.
//!
//! A field is a function of geometry and sampling policy alone. Colour,
//! opacity, material, surface pattern and visual style are applied when the
//! field is *drawn*, so two surfaces that differ only in appearance address one
//! field and generate it once. Field textures are the largest single GPU
//! allocation in the engine, which is what makes this sharing worth its
//! bookkeeping.
//!
//! The pipeline this cache fronts is
//! `generation -> component filtering -> shading`, and the key spans exactly
//! the first two stages. Everything the shading stage reads lives in the
//! representation uniforms and is not part of the key.
//!
//! What a key resolves to — the textures, bind groups and probe table — is in
//! [`super::surface_field`], together with the key itself.

use super::surface_field::SharedField;
use molgfx_gpu::Device;

/// Key-sorted cache of shared surface fields.
#[derive(Debug)]
pub(crate) struct SurfaceFieldCache<D: Device> {
    entries: Vec<(super::surface_field::SurfaceFieldKey, SharedField<D>)>,
}

impl<D: Device> SurfaceFieldCache<D> {
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Populates one field, building it on first use.
    ///
    /// A second surface over the same key takes the existing field untouched,
    /// so only the first one allocates and only the first one generates.
    pub(super) fn prepare(
        &mut self,
        sync: &super::surface_field::FieldSync<'_, D>,
    ) -> Result<(), crate::error::RenderError> {
        let Err(position) = self
            .entries
            .binary_search_by_key(&sync.key, |(key, _)| *key)
        else {
            return Ok(());
        };
        let field = super::surface_field::build_field(sync)?;
        self.entries.insert(position, (sync.key, field));
        Ok(())
    }

    /// Releases every field no surface asked for this frame.
    pub(super) fn retain(&mut self, live: &[super::surface_field::SurfaceFieldKey]) {
        self.entries.retain(|(key, _)| live.contains(key));
    }

    /// The resident field for `key`, if any.
    pub(super) fn get(
        &self,
        key: super::surface_field::SurfaceFieldKey,
    ) -> Option<&SharedField<D>> {
        let position = self
            .entries
            .binary_search_by_key(&key, |(candidate, _)| *candidate)
            .ok()?;
        self.entries.get(position).map(|(_, field)| field)
    }

    /// The mutable field for `key`, for its generation pass.
    pub(super) fn get_mut(
        &mut self,
        key: super::surface_field::SurfaceFieldKey,
    ) -> Option<&mut SharedField<D>> {
        let position = self
            .entries
            .binary_search_by_key(&key, |(candidate, _)| *candidate)
            .ok()?;
        self.entries.get_mut(position).map(|(_, field)| field)
    }

    /// Device bytes held by every resident field.
    #[must_use]
    pub(crate) fn resident_bytes(&self) -> u64 {
        self.entries.iter().fold(0u64, |total, (_, field)| {
            total.saturating_add(field.resident_bytes())
        })
    }
}

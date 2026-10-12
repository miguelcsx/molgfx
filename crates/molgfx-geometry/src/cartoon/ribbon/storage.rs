//! Reusable geometry storage and borrowed GPU upload views.
use super::{RibbonDeformation, RibbonMesh};

impl RibbonMesh {
    /// Empties all generated columns so reused scratch cannot retain unrelated recipes.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.deformations.clear();
        self.traces.clear();
        self.fit_jobs.clear();
        self.classic = false;
        self.strand_fits.clear();
    }

    /// Pads caller-appended static geometry so every vertex has one recipe.
    pub fn pad_static_deformations(&mut self) {
        self.deformations
            .resize(self.vertices.len(), RibbonDeformation::STATIC);
    }

    /// Packed GPU deformation recipes, one 32-byte row per vertex.
    #[must_use]
    pub fn deformation_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.deformations)
    }

    /// Compact B-factor stream aligned with extracted backbone guides.
    ///
    /// The stream contains no coordinates; deformation reads those directly
    /// from the structure asset's borrowed coordinate binding.
    #[must_use]
    pub fn radius_source_values(&self) -> &[f32] {
        &self.traces.properties
    }
}

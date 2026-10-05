//! Immutable native point scenes, independent of molecular atom tables.

use crate::{Color, Error};
use molgfx_core::{PointBatch, PointGlyph, PointSampling, PointStyle, SourceNamespace, SourceRows};
use std::sync::Arc;

/// A resident point scene with compact shared positions and batch-wide colours.
///
/// Construction copies no coordinates. Rendering retains the scene and its GPU
/// cache identity; points are never expanded into fictitious molecular atoms.
#[derive(Debug)]
pub struct PointCloud {
    pub(super) scene: molgfx_core::Scene,
    count: usize,
}

impl PointCloud {
    /// Builds colour groups with one world-space radius and stable row order.
    ///
    /// # Errors
    /// Returns an error for empty groups, invalid positions/radius or row overflow.
    pub fn new(groups: Vec<(Arc<[[f32; 3]]>, Color)>, radius: f32) -> Result<Self, Error> {
        if groups.is_empty() {
            return Err(Error::InvalidSpec(
                "point cloud requires a colour group".into(),
            ));
        }
        let mut scene = molgfx_core::Scene::new();
        let mut count = 0;
        for (index, (positions, color)) in groups.into_iter().enumerate() {
            let rows = u32::try_from(positions.len())
                .map_err(|_| Error::InvalidSpec("point group exceeds row addressing".into()))?;
            let namespace = u64::try_from(index)
                .map_err(|_| Error::InvalidSpec("point cloud exceeds group addressing".into()))?;
            count += positions.len();
            let batch = PointBatch::new(
                positions,
                SourceRows::ordered(SourceNamespace(namespace), rows),
                PointGlyph::Disc,
                PointStyle {
                    radius,
                    color: color.native(),
                },
            )?;
            let _handle = scene.add_point_batch(batch.with_sampling(PointSampling::All));
        }
        Ok(Self { scene, count })
    }

    /// Number of points across all groups.
    #[must_use]
    pub const fn point_count(&self) -> usize {
        self.count
    }

    /// Composes shared point batches with a snapshot of a molecular scene.
    ///
    /// Subsequent edits to the source scene do not change this composition.
    /// Molecular assets and point positions remain shared, not materialised.
    ///
    /// # Errors
    /// Returns an error when the molecular scene cannot be resolved.
    pub fn with_scene(&self, scene: &crate::Scene) -> Result<Self, Error> {
        let mut resolved = scene.resolved_snapshot()?;
        for (_, batch) in self.scene.point_batches() {
            let _handle = resolved.add_point_batch(batch.clone());
        }
        Ok(Self {
            scene: resolved,
            count: self.count,
        })
    }
}

impl super::Renderer {
    /// Renders a persistent native point scene with an explicit framing camera.
    ///
    /// # Errors
    /// Returns an error for an empty extent or a rendering/device failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_cloud_image(
        &mut self,
        cloud: &PointCloud,
        camera: &crate::Camera,
        size: (u32, u32),
    ) -> Result<super::Image, Error> {
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::InvalidSpec(
                "image width and height must be non-zero".into(),
            ));
        }
        let image = self
            .inner
            .render_image(
                &cloud.scene,
                camera,
                molgfx_render::ImageConfig {
                    width: size.0,
                    height: size.1,
                },
            )
            .map(super::Image)?;
        self.pick_source_id = Some(cloud.scene.cache_identity());
        Ok(image)
    }
}

#[cfg(test)]
#[path = "point_cloud_tests.rs"]
mod tests;

//! Portable field-line controls and lowering onto the analytic relation stream.

use crate::{Color, Error};
use molgfx_geometry::FieldLineParams;
use serde::{Deserialize, Serialize};

/// Gradient-streamline authoring over a scalar-grid binding.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldLines {
    /// Seed stride on each voxel axis.
    pub seed_stride: u32,
    /// Inclusive gradient-magnitude interval selecting seeds.
    pub seed_gradient: [f32; 2],
    /// Integration step in world ångström.
    pub step: f32,
    /// Inclusive displayed arclength interval in world ångström.
    pub length: [f32; 2],
    /// Critical gradient-magnitude termination threshold.
    pub critical_gradient: f32,
    /// Integration steps allowed on each side of a seed.
    pub max_steps: u32,
    /// Explicit output segment budget.
    pub max_segments: u32,
    /// Physical stroke width in pixels.
    pub width_pixels: f32,
    /// Stroke color.
    pub color: Color,
    /// Stroke opacity in zero to one.
    pub opacity: f32,
}

impl Default for FieldLines {
    fn default() -> Self {
        Self {
            seed_stride: 4,
            seed_gradient: [0.0, f32::MAX],
            step: 0.25,
            length: [1.0, 50.0],
            critical_gradient: 1.0e-6,
            max_steps: 1024,
            max_segments: 1_000_000,
            width_pixels: 1.5,
            color: Color::rgb(77, 179, 255),
            opacity: 1.0,
        }
    }
}

impl FieldLines {
    pub(crate) fn parameters(self) -> Result<FieldLineParams, Error> {
        if !self.width_pixels.is_finite()
            || self.width_pixels <= 0.0
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
        {
            return Err(Error::InvalidSpec(
                "field-line width must be positive and opacity between zero and one".to_owned(),
            ));
        }
        let params = FieldLineParams {
            seed_stride: self.seed_stride,
            seed_gradient: self.seed_gradient,
            step: self.step,
            length: self.length,
            critical_gradient: self.critical_gradient,
            max_steps: self.max_steps,
            max_segments: usize::try_from(self.max_segments).map_err(|_| {
                Error::InvalidSpec("field-line segment budget exceeds addressable rows".to_owned())
            })?,
        };
        params
            .validate()
            .map_err(|error| Error::InvalidSpec(error.to_string()))?;
        Ok(params)
    }
}

pub(super) fn lower(
    scene: &mut molgfx_core::Scene,
    volume: &molgfx_core::ScalarVolume,
    options: FieldLines,
    region: Option<molgfx_core::VolumeRegion>,
    namespace: u64,
) -> Result<(), Error> {
    use molgfx_core::{
        Relation, RelationBatch, RelationStyle, SourceNamespace, SourceRows, SpatialAnchor,
    };
    let paths = molgfx_geometry::field_lines_in_region(volume, options.parameters()?, region)
        .map_err(|error| Error::InvalidSpec(error.to_string()))?;
    let mut relations = Vec::new();
    for path in paths {
        for pair in path.windows(2) {
            relations.push(Relation {
                start: SpatialAnchor::world(pair[0])?,
                end: SpatialAnchor::world(pair[1])?,
            });
        }
    }
    if relations.is_empty() {
        return Ok(());
    }
    let count = u32::try_from(relations.len())
        .map_err(|_| Error::InvalidSpec("field-line count exceeds addressable rows".to_owned()))?;
    let batch = RelationBatch::new(
        std::sync::Arc::from(relations),
        SourceRows::ordered(SourceNamespace(namespace), count),
        RelationStyle {
            color: options.color.native(),
            opacity: options.opacity,
            width_pixels: options.width_pixels,
            ..RelationStyle::default()
        },
    )?;
    let _ = scene.add_relation_batch(batch)?;
    Ok(())
}

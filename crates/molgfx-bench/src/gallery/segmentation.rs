//! Two categorical maps share labels while retaining independent styles and identities.

use super::catalog::{Fixture, Result};
use molgfx::{Color, Scene, SegmentStyle, SegmentationBinding, SegmentationSpec};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io, path::Path, sync::Arc};

#[cfg(test)]
#[path = "segmentation_tests.rs"]
mod tests;

#[derive(Deserialize)]
struct CaseMap {
    name: String,
    translation: [f32; 3],
    styles: Vec<CaseStyle>,
}

#[derive(Deserialize)]
struct CaseStyle {
    label: u32,
    color_rgb: [u8; 3],
    opacity: f32,
    visible: bool,
}

/// Builds independent styled categorical maps from canonical source values.
///
/// # Errors
/// Rejects malformed maps, categorical recipes and scene bindings.
pub fn segmentation_scene(fixture: &Fixture, cache: &Path) -> Result<(Scene, Value)> {
    let (map, affine) = crate::reader::read_density_map(&cache.join(&fixture.file))?;
    let maps: Vec<CaseMap> = serde_json::from_value(
        fixture
            .details
            .get("segmentations")
            .ok_or_else(|| io::Error::other("missing categorical maps"))?
            .clone(),
    )?;
    let thresholds: [f32; 3] = serde_json::from_value(
        fixture
            .details
            .get("segmentation_thresholds")
            .ok_or_else(|| io::Error::other("missing categorical thresholds"))?
            .clone(),
    )?;
    let labels: Arc<[u32]> = map
        .values
        .iter()
        .map(|value| label(*value, thresholds))
        .collect();
    let dimensions = [
        u32::try_from(map.dimensions[0])?,
        u32::try_from(map.dimensions[1])?,
        u32::try_from(map.dimensions[2])?,
    ];
    let mut scene = Scene::empty();
    let mut identities = Vec::with_capacity(maps.len());
    for map in maps {
        let mut transform = affine;
        for axis in 0..3 {
            transform[12 + axis] += map.translation[axis];
        }
        let source =
            molgfx::schema::DataSource::new(format!("{}:categorical:{}", fixture.sha256, map.name))
                .format("labels");
        let styles = map
            .styles
            .into_iter()
            .map(|style| SegmentStyle {
                label: style.label,
                color: Color::rgb(style.color_rgb[0], style.color_rgb[1], style.color_rgb[2]),
                opacity: style.opacity,
                visible: style.visible,
            })
            .collect();
        let id = scene.add_segmentation(SegmentationSpec {
            presentation: molgfx::SegmentationPresentation::Surface,
            source: source.clone(),
            dimensions,
            voxel_to_world: transform,
            styles,
        })?;
        scene.bind_segmentation(SegmentationBinding::new(
            source,
            dimensions,
            transform,
            Arc::clone(&labels),
        )?)?;
        identities.push(json!({"name":map.name,"id":id,"voxel_to_world":transform}));
    }
    Ok((
        scene,
        json!({"atoms":0,"source_sha256":fixture.sha256,"dimensions":dimensions,
        "segmentation_thresholds":thresholds,"segmentations":identities,
        "label_policy":"descending thresholds map to labels 1, 2, 3; below all thresholds is label 0"}),
    ))
}

fn label(value: f32, thresholds: [f32; 3]) -> u32 {
    if value >= thresholds[2] {
        1
    } else if value >= thresholds[1] {
        2
    } else if value >= thresholds[0] {
        3
    } else {
        0
    }
}

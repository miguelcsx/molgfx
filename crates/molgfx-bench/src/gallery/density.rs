//! Shared scalar-volume gallery recipes.
use super::catalog::{Fixture, Result};
use molgfx::{Color, Scene, rep};
use serde_json::{Value, json};
use std::{io, path::Path};

/// Loads `MolFrame`'s canonical X-fastest scalar grid and preserves its cell affine.
///
/// # Errors
/// Rejects invalid scalar inputs, presentation parameters and source bindings.
pub fn density(fixture: &Fixture, cache: &Path) -> Result<(Scene, Value)> {
    use num_traits::ToPrimitive as _;
    let (map, affine) = crate::reader::read_density_map(&cache.join(&fixture.file))?;
    let value_range = map
        .values
        .iter()
        .copied()
        .fold([f32::INFINITY, f32::NEG_INFINITY], |range, value| {
            [range[0].min(value), range[1].max(value)]
        });
    let dimensions = [
        u32::try_from(map.dimensions[0])?,
        u32::try_from(map.dimensions[1])?,
        u32::try_from(map.dimensions[2])?,
    ];
    let level = fixture
        .details
        .get("isovalue")
        .and_then(Value::as_f64)
        .and_then(|value| value.to_f32())
        .ok_or_else(|| io::Error::other("density fixture requires a finite isovalue"))?;
    let color = Color::rgb(49, 104, 142);
    let opacity = fixture.details.get("opacity").map_or(Ok(1.0), |value| {
        value
            .as_f64()
            .and_then(|value| value.to_f32())
            .ok_or_else(|| io::Error::other("density opacity must be a finite number"))
    })?;
    let source = molgfx::schema::DataSource::new(fixture.sha256.clone()).format("mrc");
    let mut volume = molgfx::density::volume(source.clone(), dimensions).affine(affine);
    volume = match fixture.form.as_str() {
        "iso_mesh" => {
            volume.isosurface_mesh(level, color, 1.0, width(fixture, "line_width_voxels")?)
        }
        "iso_dots" => {
            volume.isosurface_dots(level, color, 1.0, width(fixture, "dot_radius_voxels")?)
        }
        "direct" => volume.direct(
            vec![
                molgfx::VolumeTransferPoint {
                    value: 0.0,
                    color,
                    opacity: 0.0,
                },
                molgfx::VolumeTransferPoint {
                    value: level * 2.0,
                    color,
                    opacity: 0.8,
                },
            ],
            2.0,
            0.65,
        ),
        "slice" => {
            let point = vector(fixture, "slice_point")?;
            volume.slice(
                point,
                vector(fixture, "slice_normal")?,
                "viridis",
                [0.0, level * 2.0],
            )
        }
        "region" => volume
            .isosurface(level, color, 1.0)
            .region([0; 3], dimensions.map(|value| value / 2)),
        "isosurface" | "script" => volume.isosurface(level, color, opacity),
        other => return Err(io::Error::other(format!("unknown density recipe: {other}")).into()),
    };
    let (mut scene, atoms) = if let Some(file) = fixture
        .details
        .get("structure_file")
        .and_then(Value::as_str)
    {
        let (structure, _) = crate::reader::read_structure(&cache.join(file))?;
        let mut scene = Scene::from_structure(&structure)?;
        let _ = scene.add(rep::cartoon("protein"))?;
        (scene, structure.atom_count())
    } else {
        (Scene::empty(), 0)
    };
    let _ = scene.add(volume)?;
    scene.bind_volume(
        molgfx::VolumeBinding::new(source, dimensions, std::sync::Arc::from(map.values))
            .affine(affine),
    )?;
    Ok((
        scene,
        json!({"atoms":atoms,"dimensions":dimensions,"voxel_to_world":affine,"value_range":value_range,
            "source_sha256":fixture.sha256}),
    ))
}

fn width(fixture: &Fixture, key: &str) -> Result<f32> {
    use num_traits::ToPrimitive as _;
    fixture
        .details
        .get(key)
        .and_then(Value::as_f64)
        .and_then(|value| value.to_f32())
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| io::Error::other(format!("density fixture requires positive {key}")).into())
}

fn vector(fixture: &Fixture, key: &str) -> Result<[f32; 3]> {
    let value = fixture
        .details
        .get(key)
        .ok_or_else(|| io::Error::other(format!("density fixture requires {key}")))?;
    let vector: [f32; 3] = serde_json::from_value(value.clone())?;
    if vector.iter().any(|value| !value.is_finite()) {
        return Err(io::Error::other(format!("density {key} must be finite")).into());
    }
    Ok(vector)
}

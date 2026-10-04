//! Curated affine-volume command payloads.
use molgfx_scene::VolumeSpec;
/// Parses the scene JSON form or explicit source/dims/affine/presentation fields.
pub(crate) fn parse(payload: &str) -> Result<VolumeSpec, String> {
    if payload.starts_with('{') {
        return serde_json::from_str(payload).map_err(|error| error.to_string());
    }
    let rest = payload.strip_prefix("source ").ok_or_else(|| {
        "volume requires source HASH dims [X,Y,Z] affine [16 values] iso|direct|slice JSON"
            .to_owned()
    })?;
    let (hash, rest) = rest
        .split_once(" dims ")
        .ok_or_else(|| "volume is missing dims".to_owned())?;
    let (dims, rest) = rest
        .split_once(" affine ")
        .ok_or_else(|| "volume is missing affine".to_owned())?;
    let (affine, presentation) = rest
        .split_once(']')
        .ok_or_else(|| "volume affine must be a JSON array".to_owned())?;
    let dimensions =
        serde_json::from_str(dims).map_err(|error| format!("invalid dimensions: {error}"))?;
    let affine = format!("{affine}]");
    let voxel_to_world =
        serde_json::from_str(&affine).map_err(|error| format!("invalid affine: {error}"))?;
    let (kind, json) = presentation
        .trim()
        .split_once(' ')
        .ok_or_else(|| "volume needs an iso, direct, or slice presentation".to_owned())?;
    let kind = match kind {
        "iso" => "isosurface",
        "direct" => "direct",
        "slice" => "slice",
        _ => return Err("unknown volume presentation".to_owned()),
    };
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "volume presentation must be a JSON object".to_owned())?
        .insert(
            "kind".to_owned(),
            serde_json::Value::String(kind.to_owned()),
        );
    Ok(VolumeSpec {
        source: molgfx_scene::DataSource::new(hash.trim()),
        dimensions,
        voxel_to_world,
        presentations: vec![serde_json::from_value(value).map_err(|error| error.to_string())?],
        region: None,
    })
}

#[cfg(test)]
#[path = "volume_tests.rs"]
mod tests;

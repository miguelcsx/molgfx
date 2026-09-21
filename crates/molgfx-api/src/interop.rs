//! `MolViewSpec` v1 adapter around the stable [`crate::SceneSpec`] contract.

use crate::SceneSpec;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::io::{Cursor, Read, Write};

#[path = "interop_import.rs"]
mod import;
#[path = "interop_schema.rs"]
mod schema;

/// Information that could not be represented exactly during interchange.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable machine-readable code.
    pub code: Box<str>,
    /// Human-readable explanation.
    pub message: Box<str>,
    /// Tree path of the affected `MolViewSpec` node.
    pub path: Box<str>,
}

/// Generic exhaustive `MolViewSpec` node envelope.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MvsNode {
    /// Node kind from the v1 schema.
    pub kind: Box<str>,
    /// Kind-specific parameters, preserved even when unsupported.
    #[serde(default)]
    pub params: Map<String, Value>,
    /// Ordered children.
    #[serde(default)]
    pub children: Vec<MvsNode>,
}

/// `MolViewSpec` v1 JSON document.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MvsDocument {
    /// Specification metadata.
    #[serde(default)]
    pub metadata: Map<String, Value>,
    /// Required root node.
    pub root: MvsNode,
}

/// Imported scene plus loss diagnostics.
#[derive(Clone, PartialEq, Debug)]
pub struct MvsImport {
    /// Portable `MolGFX` semantic scene.
    pub scene: SceneSpec,
    /// Unsupported or approximated source information.
    pub diagnostics: Vec<Diagnostic>,
}

/// Exports the compatible semantic subset as deterministic `.mvsj` JSON.
///
/// # Errors
///
/// Returns an encoding error if the semantic document cannot be serialized.
pub fn to_mvsj(scene: &SceneSpec) -> Result<String, crate::Error> {
    serde_json::to_string(&export_document(scene)).map_err(crate::Error::from)
}

/// Imports an `.mvsj`, preserving unsupported data in diagnostics.
///
/// # Errors
///
/// Returns an error for malformed JSON or a document without a root node.
pub fn from_mvsj(source: &str) -> Result<MvsImport, crate::Error> {
    let document: MvsDocument = serde_json::from_str(source)?;
    import::import_document(&document)
}

/// Encodes a standard ZIP-based `.mvsx` container with `index.mvsj`.
///
/// # Errors
///
/// Returns an encoding or archive error if the container cannot be written.
pub fn to_mvsx(scene: &SceneSpec) -> Result<Vec<u8>, crate::Error> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file("index.mvsj", zip::write::SimpleFileOptions::default())
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))?;
    archive
        .write_all(to_mvsj(scene)?.as_bytes())
        .map_err(crate::Error::from)?;
    archive
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))
}

/// Imports `index.mvsj` from a standard `.mvsx` ZIP container.
///
/// # Errors
///
/// Returns an error for an invalid archive, missing index, or malformed JSON.
pub fn from_mvsx(bytes: &[u8]) -> Result<MvsImport, crate::Error> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))?;
    let mut source = String::new();
    archive
        .by_name("index.mvsj")
        .map_err(|error| crate::Error::InvalidSpec(error.to_string()))?
        .read_to_string(&mut source)
        .map_err(crate::Error::from)?;
    from_mvsj(&source)
}

fn export_document(scene: &SceneSpec) -> MvsDocument {
    let mut root = node("root", &json!({}));
    for (id, source) in &scene.structures {
        let uri = crate::fallback(source.uri.as_deref(), "");
        let format = crate::fallback(source.format.as_deref(), "mmcif");
        let mut download = node("download", &json!({ "url": uri }));
        let mut parse = node("parse", &json!({ "format": format }));
        let mut structure = node("structure", &json!({ "type": "model" }));
        for (representation_id, representation) in &scene.representations {
            if representation.structure_id() != Some(*id) {
                continue;
            }
            let mut component = node(
                "component",
                &json!({ "selector": representation.selection() }),
            );
            let encoded = crate::fallback(serde_json::to_value(representation), Value::Null);
            let kind = crate::fallback(encoded.get("form").and_then(Value::as_str), "cartoon");
            let mut visual = node(
                "representation",
                &json!({ "type": mvs_representation(kind) }),
            );
            if let Some(color) = uniform_hex(&encoded) {
                visual
                    .children
                    .push(node("color", &json!({ "color": color })));
            }
            if let Some(opacity) = encoded.get("opacity").and_then(Value::as_f64) {
                visual
                    .children
                    .push(node("opacity", &json!({ "opacity": opacity })));
            }
            visual
                .params
                .insert("molgfx_id".to_owned(), json!(representation_id.get()));
            component.children.push(visual);
            structure.children.push(component);
        }
        structure
            .params
            .insert("molgfx_structure_id".to_owned(), json!(id.get()));
        structure
            .params
            .insert("molgfx_content_hash".to_owned(), json!(source.content_hash));
        parse.children.push(structure);
        download.children.push(parse);
        root.children.push(download);
    }
    if let Some(focus) = &scene.focus {
        root.children
            .push(node("focus", &json!({ "selector": focus.source() })));
    }
    if let Some(camera) = scene.camera {
        root.children.push(node(
            "camera",
            &json!({
                "target": vector(camera.target),
                "position": vector(camera.eye),
                "up": vector(camera.up),
            }),
        ));
    } else if let Some(camera) = scene.extensions.get("org.molgfx.mvs.camera") {
        root.children.push(node("camera", camera));
    }
    if let Some(annotations) = scene
        .extensions
        .get("org.molgfx.mvs.annotations")
        .and_then(Value::as_array)
    {
        root.children.extend(
            annotations
                .iter()
                .filter_map(|value| serde_json::from_value::<MvsNode>(value.clone()).ok()),
        );
    }
    MvsDocument {
        metadata: Map::from_iter([("version".to_owned(), json!(1))]),
        root,
    }
}

fn vector(value: molgfx_math::Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn node(kind: &str, params: &Value) -> MvsNode {
    let params = match params.as_object() {
        Some(value) => value.clone(),
        None => Map::new(),
    };
    MvsNode {
        kind: kind.into(),
        params,
        children: Vec::new(),
    }
}

fn mvs_representation(kind: &str) -> &str {
    match kind {
        "lines" => "line",
        "licorice" => "ball_and_stick",
        "points" => "spacefill",
        other => other,
    }
}

fn uniform_hex(value: &Value) -> Option<String> {
    let lanes = value.get("color")?.get("color")?.as_array()?;
    Some(format!(
        "#{:02X}{:02X}{:02X}",
        lanes.first()?.as_u64()?,
        lanes.get(1)?.as_u64()?,
        lanes.get(2)?.as_u64()?
    ))
}

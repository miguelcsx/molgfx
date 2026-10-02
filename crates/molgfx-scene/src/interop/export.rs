//! Lowering a scene specification to a `MolViewSpec` document.

use super::{MvsDocument, MvsNode};
use crate::SceneSpec;
use serde_json::{Map, Value, json};

pub(super) fn export_document(scene: &SceneSpec) -> MvsDocument {
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

pub(super) fn vector(value: molgfx_math::Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

pub(super) fn node(kind: &str, params: &Value) -> MvsNode {
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

pub(super) fn mvs_representation(kind: &str) -> &str {
    match kind {
        "lines" => "line",
        "licorice" => "ball_and_stick",
        "points" => "spacefill",
        other => other,
    }
}

pub(super) fn uniform_hex(value: &Value) -> Option<String> {
    let lanes = value.get("color")?.get("color")?.as_array()?;
    Some(format!(
        "#{:02X}{:02X}{:02X}",
        lanes.first()?.as_u64()?,
        lanes.get(1)?.as_u64()?,
        lanes.get(2)?.as_u64()?
    ))
}

//! `MolViewSpec` tree traversal and semantic import.

use super::{Diagnostic, MvsDocument, MvsImport, MvsNode, schema};
use crate::{RepresentationId, SceneSpec, StructureId, StructureSource, rep};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(crate) fn import_document(document: &MvsDocument) -> Result<MvsImport, crate::Error> {
    if document.root.kind.as_ref() != "root" {
        return Err(crate::Error::InvalidSpec(
            "MolViewSpec root node is required".to_owned(),
        ));
    }
    let mut state = ImportState::new();
    walk(&document.root, "root", &mut |node, path, ancestors| {
        state.visit(node, path, ancestors);
    });
    state.finish(document)
}

struct ImportState {
    scene: SceneSpec,
    diagnostics: Vec<Diagnostic>,
    next_structure: u64,
    next_representation: u64,
    structure_ids: BTreeMap<*const MvsNode, StructureId>,
}

impl ImportState {
    fn new() -> Self {
        Self {
            scene: SceneSpec::empty(),
            diagnostics: Vec::new(),
            next_structure: 1,
            next_representation: 1,
            structure_ids: BTreeMap::new(),
        }
    }

    fn visit(&mut self, node: &MvsNode, path: &str, ancestors: &[&MvsNode]) {
        match node.kind.as_ref() {
            "structure" => self.add_structure(node, ancestors),
            "representation" => self.add_representation(node, path, ancestors),
            "focus" => self.set_focus(node),
            "camera" => self.set_camera(node),
            "label"
            | "label_from_uri"
            | "label_from_source"
            | "tooltip"
            | "tooltip_from_uri"
            | "tooltip_from_source" => self.add_annotation(node),
            _ => self.diagnose_node(node, path),
        }
    }

    fn add_structure(&mut self, node: &MvsNode, ancestors: &[&MvsNode]) {
        let raw_id = semantic_id(node, "molgfx_structure_id", self.next_structure);
        self.next_structure = self.next_structure.max(raw_id.saturating_add(1));
        let id = StructureId::new(raw_id);
        let _ = self.structure_ids.insert(std::ptr::from_ref(node), id);
        let hash = crate::fallback(
            node.params
                .get("molgfx_content_hash")
                .and_then(Value::as_str),
            "",
        );
        let _ = self.scene.structures.insert(
            id,
            StructureSource {
                content_hash: hash.into(),
                uri: ancestor_param(ancestors, "download", "url").map(Into::into),
                format: ancestor_param(ancestors, "parse", "format").map(Into::into),
            },
        );
    }

    fn add_representation(&mut self, node: &MvsNode, path: &str, ancestors: &[&MvsNode]) {
        let selector_value = ancestors
            .iter()
            .rev()
            .find(|parent| parent.kind.as_ref() == "component")
            .and_then(|component| component.params.get("selector"));
        let selector = match schema::molecular_selector(selector_value) {
            Ok(selector) => selector,
            Err(message) => {
                self.diagnostics.push(Diagnostic {
                    code: "unsupported_selector".into(),
                    message: message.into(),
                    path: path.into(),
                });
                "none".into()
            }
        };
        let kind = crate::fallback(node.params.get("type").and_then(Value::as_str), "cartoon");
        let mut spec = representation(kind, &selector, &node.children, path, &mut self.diagnostics);
        spec.common.structure = ancestors
            .iter()
            .rev()
            .find(|parent| parent.kind.as_ref() == "structure")
            .and_then(|parent| {
                self.structure_ids
                    .get(&std::ptr::from_ref(*parent))
                    .copied()
            });
        let raw_id = semantic_id(node, "molgfx_id", self.next_representation);
        self.next_representation = self.next_representation.max(raw_id.saturating_add(1));
        let _ = self
            .scene
            .representations
            .insert(RepresentationId::new(raw_id), spec);
    }

    fn set_focus(&mut self, node: &MvsNode) {
        if let Some(selector) = node.params.get("selector").and_then(Value::as_str) {
            self.scene.focus = Some(selector.into());
        }
    }

    fn set_camera(&mut self, node: &MvsNode) {
        self.scene.camera = mvs_camera(&node.params);
        self.scene.extensions.insert(
            "org.molgfx.mvs.camera".into(),
            Value::Object(node.params.clone()),
        );
    }

    fn add_annotation(&mut self, node: &MvsNode) {
        let annotations = self
            .scene
            .extensions
            .entry("org.molgfx.mvs.annotations".into())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(values) = annotations
            && let Ok(value) = serde_json::to_value(node)
        {
            values.push(value);
        }
    }

    fn diagnose_node(&mut self, node: &MvsNode, path: &str) {
        let diagnostic = match schema::classify(&node.kind) {
            schema::NodeSupport::Supported => None,
            schema::NodeSupport::Unsupported => Some((
                "unsupported_node",
                "is valid MolViewSpec v1 but is preserved only in the source extension",
            )),
            schema::NodeSupport::Unknown => {
                Some(("unknown_node", "is not a MolViewSpec v1 node kind"))
            }
        };
        if let Some((code, message)) = diagnostic {
            self.diagnostics.push(Diagnostic {
                code: code.into(),
                message: format!("MolViewSpec node '{}' {message}", node.kind).into(),
                path: path.into(),
            });
        }
    }

    fn finish(mut self, document: &MvsDocument) -> Result<MvsImport, crate::Error> {
        self.scene.extensions.insert(
            "org.molgfx.mvs.metadata".into(),
            Value::Object(document.metadata.clone()),
        );
        self.scene.extensions.insert(
            "org.molgfx.mvs.source".into(),
            serde_json::to_value(document)?,
        );
        Ok(MvsImport {
            scene: self.scene,
            diagnostics: self.diagnostics,
        })
    }
}

fn mvs_camera(params: &Map<String, Value>) -> Option<molgfx_math::Camera> {
    let target = vector_param(params, "target")?;
    let eye = vector_param(params, "position")?;
    let up = vector_param(params, "up")
        .into_iter()
        .fold(molgfx_math::Vec3::Y, |_, value| value);
    let declared_near = params
        .get("near")
        .and_then(Value::as_f64)
        .and_then(|value| value.to_string().parse::<f32>().ok());
    let near = crate::fallback(declared_near, 0.1).max(0.01);
    let far = eye.distance(target).mul_add(4.0, near).max(near + 1.0);
    Some(molgfx_math::Camera {
        eye,
        target,
        up,
        projection: molgfx_math::Projection::Perspective {
            fov_y: std::f32::consts::FRAC_PI_4,
            aspect: 1.0,
            near,
            far,
        },
    })
}

fn vector_param(params: &Map<String, Value>, name: &str) -> Option<molgfx_math::Vec3> {
    let values = params.get(name)?.as_array()?;
    Some(molgfx_math::Vec3::new(
        number(values.first()?)?,
        number(values.get(1)?)?,
        number(values.get(2)?)?,
    ))
}

fn number(value: &Value) -> Option<f32> {
    value.as_f64()?.to_string().parse().ok()
}

fn representation(
    kind: &str,
    selector: &str,
    children: &[MvsNode],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> crate::RepresentationSpec {
    let mut color = None;
    let mut opacity = 1.0_f32;
    for child in children {
        if child.kind.as_ref() == "color" {
            color = child
                .params
                .get("color")
                .and_then(Value::as_str)
                .and_then(schema::parse_hex);
        } else if child.kind.as_ref() == "opacity" {
            opacity = crate::fallback(
                child
                    .params
                    .get("opacity")
                    .and_then(Value::as_f64)
                    .and_then(|value| value.to_string().parse::<f32>().ok()),
                1.0,
            );
        }
    }
    let mut spec: crate::RepresentationSpec = match kind {
        "cartoon" | "backbone" | "putty" => rep::cartoon(selector).into(),
        "ball_and_stick" => rep::ball_and_stick(selector).into(),
        "line" => rep::lines(selector).into(),
        "spacefill" => rep::spacefill(selector).into(),
        "carbohydrate" => rep::glycan(selector).into(),
        "surface" => rep::surface(selector).into(),
        other => {
            diagnostics.push(Diagnostic {
                code: "unsupported_representation".into(),
                message: format!("representation '{other}' mapped to cartoon").into(),
                path: path.into(),
            });
            rep::cartoon(selector).into()
        }
    };
    spec.common.opacity = opacity;
    if let Some(color) = color {
        spec.common.color = crate::color::uniform(color);
    }
    spec
}

fn walk<'a>(
    node: &'a MvsNode,
    path: &str,
    visit: &mut impl FnMut(&'a MvsNode, &str, &[&'a MvsNode]),
) {
    fn inner<'a>(
        node: &'a MvsNode,
        path: &str,
        parents: &mut Vec<&'a MvsNode>,
        visit: &mut impl FnMut(&'a MvsNode, &str, &[&'a MvsNode]),
    ) {
        visit(node, path, parents);
        parents.push(node);
        for (index, child) in node.children.iter().enumerate() {
            inner(
                child,
                &format!("{path}/{}[{index}]", child.kind),
                parents,
                visit,
            );
        }
        let _ = parents.pop();
    }
    inner(node, path, &mut Vec::new(), visit);
}

fn ancestor_param<'a>(ancestors: &'a [&MvsNode], kind: &str, name: &str) -> Option<&'a str> {
    ancestors
        .iter()
        .rev()
        .find(|node| node.kind.as_ref() == kind)?
        .params
        .get(name)?
        .as_str()
}

fn semantic_id(node: &MvsNode, name: &str, fallback: u64) -> u64 {
    crate::fallback(
        node.params
            .get(name)
            .and_then(Value::as_u64)
            .filter(|value| *value != 0),
        fallback,
    )
}

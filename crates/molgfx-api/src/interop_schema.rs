//! Exhaustive `MolViewSpec` v1 node classification for adapter diagnostics.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NodeSupport {
    Supported,
    Unsupported,
    Unknown,
}

pub(super) fn classify(kind: &str) -> NodeSupport {
    match kind {
        "root"
        | "download"
        | "parse"
        | "structure"
        | "component"
        | "representation"
        | "color"
        | "opacity"
        | "focus"
        | "camera"
        | "label"
        | "label_from_uri"
        | "label_from_source"
        | "tooltip"
        | "tooltip_from_uri"
        | "tooltip_from_source" => NodeSupport::Supported,
        "coordinates"
        | "transform"
        | "instance"
        | "component_from_uri"
        | "component_from_source"
        | "volume"
        | "volume_representation"
        | "color_from_uri"
        | "color_from_source"
        | "clip"
        | "primitives"
        | "primitives_from_uri"
        | "primitive"
        | "canvas"
        | "transition" => NodeSupport::Unsupported,
        _ => NodeSupport::Unknown,
    }
}

pub(super) fn molecular_selector(
    value: Option<&serde_json::Value>,
) -> Result<Box<str>, &'static str> {
    let Some(value) = value else {
        return Err("component selector is required");
    };
    let Some(selector) = value.as_str() else {
        return Err("structured component selectors are not supported");
    };
    match selector {
        "branched" => Ok("saccharide".into()),
        "coarse" => Err("coarse components have no atom-level MolGFX equivalent"),
        other => Ok(other.into()),
    }
}

pub(super) fn parse_hex(source: &str) -> Option<crate::Color> {
    let source = source.strip_prefix('#')?;
    if source.len() != 6 {
        return None;
    }
    Some(crate::Color::rgb(
        u8::from_str_radix(&source[0..2], 16).ok()?,
        u8::from_str_radix(&source[2..4], 16).ok()?,
        u8::from_str_radix(&source[4..6], 16).ok()?,
    ))
}

#[cfg(test)]
#[path = "interop_schema_tests.rs"]
mod tests;

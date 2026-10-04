//! Browser names for facade-native quality recipes.

use wasm_bindgen::JsError;

/// Frame-rate target when the caller names none.
const DEFAULT_TARGET_FPS: u16 = 60;

pub(crate) fn profile(
    quality: Option<&str>,
    target_fps: Option<u16>,
    effects_json: Option<&str>,
) -> Result<molgfx::RenderProfile, JsError> {
    let fps = target_fps.map_or(DEFAULT_TARGET_FPS, u16::from);
    let mut profile = match quality {
        None | Some("auto") => molgfx::profile::adaptive(fps),
        Some("interactive") => molgfx::RenderProfile {
            quality: molgfx::Quality::Interactive,
            ..molgfx::profile::adaptive(fps)
        },
        Some("highest_fixed") => molgfx::profile::highest_fixed(fps),
        Some("converged") => molgfx::profile::converged(),
        Some(_) => {
            return Err(JsError::new(
                "quality must be auto, interactive, highest_fixed, or converged",
            ));
        }
    };
    if let Some(json) = effects_json {
        let inputs: Vec<serde_json::Value> =
            serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))?;
        for input in inputs {
            let effect = parse_effect(&input)?;
            profile = profile
                .with_effect(effect)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
    }
    Ok(profile)
}

fn parse_effect(input: &serde_json::Value) -> Result<molgfx::profile::Effect, JsError> {
    use molgfx::profile::{self, Effect};
    let name = input
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| JsError::new("effect.kind must be a string"))?;
    let settings = input
        .get("settings")
        .cloned()
        .ok_or_else(|| JsError::new("effect.settings is required"))?;

    match name {
        "depth_cue" => {
            let [near, far, strength]: [f32; 3] = decode(settings)?;
            profile::DepthCue::new(near, far, strength)
                .map(Effect::DepthCue)
                .map_err(|e| JsError::new(&e.to_string()))
        }
        "anti_aliasing" => match settings.as_str() {
            Some("off") => Ok(Effect::AntiAliasing(profile::AntiAliasing::Off)),
            Some("fxaa") => Ok(Effect::AntiAliasing(profile::AntiAliasing::Fxaa)),
            _ => Err(JsError::new("anti_aliasing settings must be off or fxaa")),
        },
        "bloom" => decode(settings).map(Effect::Bloom),
        "depth_of_field" => decode(settings).map(Effect::DepthOfField),
        "motion_blur" => decode(settings).map(Effect::MotionBlur),
        "backdrop" => decode(settings).map(Effect::Backdrop),
        "lighting" => decode(settings).map(Effect::Lighting),
        "shape_cues" => decode(settings).map(Effect::ShapeCues),
        "display" => decode(settings).map(Effect::Display),
        _ => Err(JsError::new("unknown presentation effect")),
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, JsError> {
    serde_json::from_value(value).map_err(|error| JsError::new(&error.to_string()))
}

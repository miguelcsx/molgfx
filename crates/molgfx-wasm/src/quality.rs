//! Browser names for facade-native quality recipes.

use wasm_bindgen::JsError;

pub(crate) fn profile(
    quality: Option<&str>,
    target_fps: Option<u16>,
) -> Result<molgfx::RenderProfile, JsError> {
    let fps = target_fps.unwrap_or(60);
    match quality {
        None | Some("auto") => Ok(molgfx::profile::adaptive(fps)),
        Some("interactive") => Ok(molgfx::RenderProfile {
            quality: molgfx::Quality::Interactive,
            ..molgfx::profile::adaptive(fps)
        }),
        Some("highest_fixed") => Ok(molgfx::profile::highest_fixed(fps)),
        Some("publication") => Ok(molgfx::profile::publication()),
        Some(_) => Err(JsError::new(
            "quality must be auto, interactive, highest_fixed, or publication",
        )),
    }
}

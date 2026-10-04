//! Scene composition independent of screenshot or timing orchestration.

use super::{Fixture, Result, Style, density, molecular, segmentation_scene, structure};
use molgfx::{Scene, command::Session};
use serde_json::Value;
use std::{io, path::Path};

/// Loads a complete authored gallery scene without a rendering device.
///
/// # Errors
/// Rejects malformed sources, unsupported forms and invalid command programs.
pub fn scene(fixture: &Fixture, cache: &Path, style: &Style) -> Result<(Scene, Value)> {
    if fixture.details.contains_key("segmentations") {
        segmentation_scene(fixture, cache)
    } else if fixture.script.is_some() {
        script_scene(fixture, cache)
    } else if fixture.format == "mrc" {
        density(fixture, cache)
    } else {
        let (structure, metadata) = structure(fixture, cache)?;
        let mut scene = Scene::from_structure(&structure)?;
        molecular::add_form(&mut scene, &fixture.form, style)?;
        Ok((scene, metadata))
    }
}

/// Loads a source and executes its authored commands in order.
///
/// # Errors
/// Rejects a non-script case, malformed sources and invalid commands.
pub fn script_scene(fixture: &Fixture, cache: &Path) -> Result<(Scene, Value)> {
    let script = fixture
        .script
        .as_ref()
        .ok_or_else(|| io::Error::other("script scene requested for a non-script case"))?;
    let (mut scene, metadata) = if fixture.format == "mrc" {
        density(fixture, cache)?
    } else {
        let (structure, metadata) = structure(fixture, cache)?;
        (Scene::from_structure(&structure)?, metadata)
    };
    let mut session = Session::new(&scene);
    for line in &script.molgfx {
        session
            .execute_text(&mut scene, line)
            .map_err(|errors| io::Error::other(format!("{line:?}: {}", errors.render(line))))?;
    }
    Ok((scene, metadata))
}

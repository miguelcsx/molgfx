//! Proves a cold-source render-session round trip with actual images.
//!
//! Usage: `session_smoke [structure.cif] [output-prefix]`

use pdviewx::{
    AtomSelection, Camera, ColorScheme, Engine, EngineConfig, ImageConfig, Material, RenderProfile,
    RenderSession, RepresentationKind, Rgba8, Scene, SceneDescriptionSources,
};
use std::error::Error;
use std::fs;
use std::io;

#[path = "common/mod.rs"]
mod common;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/1BNA.cif", String::as_str);
    let output_prefix = arguments
        .get(1)
        .map_or("target/visual-checks/session", String::as_str);

    let source = common::read_structure(structure_path)?;
    let mut scene = Scene::from_structure(&source)?;
    let first_structure = scene.structures().next().map(|(handle, _)| handle);
    if let Some(handle) = first_structure {
        let records = common::deposited_secondary_structure(structure_path, &source);
        if !records.is_empty() {
            scene.apply_secondary_structure(handle, &records)?;
        }
    }
    let selection = scene.add_selection(AtomSelection::All);
    let representation = scene.represent(selection, RepresentationKind::BallAndStick)?;
    let Some(value) = scene.representation_mut(representation) else {
        return Err(io::Error::other("session representation became stale").into());
    };
    value.color = ColorScheme::Uniform(Rgba8::opaque(32, 124, 210));
    value.material = Material::principled(0.18);
    value.params.radius_scale = 0.86;
    value.params.bond_radius = 0.14;

    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let profile = RenderProfile::illustrative();
    let session = RenderSession::new(&scene, camera, profile.clone());
    let json = session.to_json()?;
    let decoded = RenderSession::from_json(&json)?;
    let cold_source = common::read_structure(structure_path)?;
    let structures = [cold_source];
    let sources = SceneDescriptionSources {
        structures: &structures,
        volumes: &[],
        segmentations: &[],
        atom_properties: &[],
        meshes: &[],
    };
    let (restored, restored_camera, restored_profile) = decoded.restore(sources)?;

    let config = ImageConfig {
        width: 256,
        height: 256,
    };
    let engine_config = EngineConfig {
        profile: profile.clone(),
        ..EngineConfig::default()
    };
    let mut before_engine = Engine::new(&engine_config, None)?;
    let before = before_engine.render_image(&scene, &camera, config)?;
    let mut after_engine = Engine::new(&engine_config, None)?;
    let after = after_engine.render_image(&restored, &restored_camera, config)?;

    let scene_equal = restored.describe() == scene.describe();
    let camera_equal = restored_camera == camera;
    let profile_equal = restored_profile == profile;
    let image_equal = before.pixels == after.pixels;
    fs::write(format!("{output_prefix}-session.json"), &json)?;
    fs::write(format!("{output_prefix}-before.png"), before.png_bytes()?)?;
    fs::write(format!("{output_prefix}-after.png"), after.png_bytes()?)?;
    println!("scene_equal={scene_equal}");
    println!("camera_equal={camera_equal}");
    println!("profile_equal={profile_equal}");
    println!("image_equal={image_equal}");
    println!("session_json_bytes={}", json.len());
    if !(scene_equal && camera_equal && profile_equal && image_equal) {
        return Err(io::Error::other("session round trip changed rendered state").into());
    }
    Ok(())
}

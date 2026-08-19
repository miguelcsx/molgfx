//! Deterministic headless images for every viewer representation choice.

use super::{App, catalog, scenes};
use pdviewx::{Camera, Engine, EngineConfig, Image, ImageConfig, RepresentationKind, SurfaceStyle};
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

const IMAGE: ImageConfig = ImageConfig {
    width: 640,
    height: 480,
};
const ASPECT: f32 = 4.0 / 3.0;

pub(super) fn run(output: &str) -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Path::new(output);
    fs::create_dir_all(output)?;
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    for choice in catalog::choices() {
        let source = source_for(*choice);
        let path = root.join("benchmarks/scenes").join(source);
        let Some(path) = path.to_str() else {
            return Err(io::Error::other("audit input path is not valid UTF-8").into());
        };
        let (mut app, _) = scenes::build_plain_scene(path)
            .map_err(|error| io::Error::other(format!("{}: {error}", choice.name())))?;
        app.set_representation(*choice);
        let camera = camera(&app);
        let image = engine.render_image(&app.scene, &camera, IMAGE)?;
        let repeat = engine.render_image(&app.scene, &camera, IMAGE)?;
        if image.pixels != repeat.pixels {
            return Err(io::Error::other(format!(
                "{} is not deterministic on this adapter",
                choice.name()
            ))
            .into());
        }
        let destination = output.join(format!("{}.png", choice.name()));
        write_png(&destination, &image)?;
        println!("{:<18} {}", choice.name(), destination.display());
    }
    audit_surface_mesh(&mut engine, &root, output)?;
    audit_transitions(&root, output)?;
    Ok(())
}

fn audit_surface_mesh(
    engine: &mut Engine,
    root: &Path,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    let path = root.join("benchmarks/scenes/3PTB.cif");
    let Some(path) = path.to_str() else {
        return Err(io::Error::other("mesh audit path is not valid UTF-8").into());
    };
    let (mut app, _) = scenes::build_plain_scene(path).map_err(io::Error::other)?;
    app.set_representation(catalog::RepresentationChoice::Kind(
        RepresentationKind::Surface,
    ));
    for handle in &app.representations {
        if let Some(representation) = app.scene.representation_mut(*handle) {
            representation.params.surface_style = SurfaceStyle::Mesh;
        }
    }
    let image = engine.render_image(&app.scene, &camera(&app), IMAGE)?;
    write_png(&output.join("surface-mesh.png"), &image)
}

fn audit_transitions(root: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let path = root.join("benchmarks/scenes/3PTB.cif");
    let Some(path) = path.to_str() else {
        return Err(io::Error::other("transition input path is not valid UTF-8").into());
    };
    let (mut app, _) = scenes::build_plain_scene(path).map_err(io::Error::other)?;
    let camera = camera(&app);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    for choice in catalog::choices()
        .iter()
        .copied()
        .filter(|choice| is_protein_choice(*choice))
    {
        app.set_representation(choice);
        let image = engine.render_image(&app.scene, &camera, IMAGE)?;
        let destination = output.join(format!("transition-{}.png", choice.name()));
        write_png(&destination, &image)?;
    }
    Ok(())
}

fn is_protein_choice(choice: catalog::RepresentationChoice) -> bool {
    !matches!(
        choice,
        catalog::RepresentationChoice::Kind(
            pdviewx::RepresentationKind::Twister | pdviewx::RepresentationKind::PaperChain
        )
    )
}

fn source_for(choice: catalog::RepresentationChoice) -> &'static str {
    match choice {
        catalog::RepresentationChoice::Kind(pdviewx::RepresentationKind::Twister) => "6VXX.cif",
        catalog::RepresentationChoice::Kind(pdviewx::RepresentationKind::PaperChain) => "1BNA.cif",
        _ => "3PTB.cif",
    }
}

fn camera(app: &App) -> Camera {
    Camera::framing(&app.bound, ASPECT)
}

fn write_png(path: &PathBuf, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

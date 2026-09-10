//! Deterministic headless images for every viewer representation choice.

use super::{App, catalog, common, scenes};
use pdviewx::{
    Aabb, AtomSelection, Camera, ColorScheme, Engine, EngineConfig, Image, ImageConfig, RenderMode,
    RepresentationKind, Scene, SurfaceStyle, Vec3,
};
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

pub(super) const PREVIEW_IMAGE: ImageConfig = ImageConfig {
    width: 640,
    height: 480,
};
pub(super) const FOUR_K_IMAGE: ImageConfig = ImageConfig {
    width: 3840,
    height: 2160,
};

/// Engine for the reference stills.
///
/// Cinematic rather than realtime: these are off-screen references compared by eye
/// against other renderers, so they are accumulated over the deep subpixel
/// sequence and the sampling rate is not what a reader should be judging.
fn audit_engine() -> EngineConfig {
    EngineConfig {
        mode: RenderMode::Cinematic,
        ..EngineConfig::default()
    }
}

/// Largest per-channel difference tolerated between two renders of one scene.
///
/// Zero would be ideal, and everything the engine computes on the CPU is exactly
/// reproducible. The residue is one least-significant bit on a handful of pixels
/// where two impostors meet at equal depth: the visible-atom stream is compacted
/// by a racing atomic, so the order those two coplanar fragments reach the depth
/// test in can differ between runs and the tie resolves the other way. That is a
/// property of how the GPU retired the work, and the determinism contract binds
/// exactness above the shader, not the last bit of a rasteriser tie. A larger
/// gap would mean a real divergence and still fails.
const DETERMINISM_TOLERANCE: u8 = 1;
/// Structure carrying the deposited glycans the twister audit draws.
const GLYCAN_SOURCE: &str = "2DT3-hexasaccharide.pdb";
/// Component name of the sugar those glycans are built from.
const GLYCAN_COMPONENT: &str = "NAG";
/// Fewest linked sugars a site needs before it is a glycan rather than a lone
/// residue. One sugar has no neighbour to be oriented against, so it produces
/// no ribbon and would audit an empty image.
const MIN_GLYCAN_SUGARS: usize = 2;

pub(super) fn run(
    output: &str,
    filters: &[String],
    image_config: ImageConfig,
) -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Path::new(output);
    fs::create_dir_all(output)?;
    validate_filters(filters)?;
    let mut engine = Engine::new(&audit_engine(), None)?;
    for choice in catalog::choices()
        .iter()
        .filter(|choice| requested(**choice, filters))
    {
        let mut app = audit_app(&root, *choice)?;
        app.set_representation(*choice);
        let camera = camera_for(&app, *choice, image_aspect(image_config));
        let image = engine.render_image(&app.scene, &camera, image_config)?;
        ensure_subject(&image, choice.name())?;
        let repeat = engine.render_image(&app.scene, &camera, image_config)?;
        if let Some(delta) = max_channel_delta(&image, &repeat)
            && delta > DETERMINISM_TOLERANCE
        {
            return Err(io::Error::other(format!(
                "{} diverged by {delta} between renders on this adapter",
                choice.name()
            ))
            .into());
        }
        let destination = output.join(format!("{}.png", choice.name()));
        write_png(&destination, &image)?;
        println!("{:<18} {}", choice.name(), destination.display());
    }
    if !filters.is_empty() {
        return Ok(());
    }
    audit_surface_mesh(&mut engine, &root, output)?;
    audit_transitions(&root, output)?;
    Ok(())
}

/// Largest absolute per-byte difference between two renders, or nothing when
/// their buffers are not even the same length.
fn max_channel_delta(left: &Image, right: &Image) -> Option<u8> {
    if left.pixels.len() != right.pixels.len() {
        return Some(u8::MAX);
    }
    left.pixels
        .iter()
        .zip(&right.pixels)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
}

fn requested(choice: catalog::RepresentationChoice, filters: &[String]) -> bool {
    filters.is_empty() || filters.iter().any(|filter| filter == choice.name())
}

fn validate_filters(filters: &[String]) -> Result<(), io::Error> {
    for filter in filters {
        if !catalog::choices()
            .iter()
            .any(|choice| choice.name() == filter)
        {
            return Err(io::Error::other(format!(
                "unknown representation audit filter '{filter}'"
            )));
        }
    }
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
    let image = engine.render_image(
        &app.scene,
        &camera(&app, image_aspect(PREVIEW_IMAGE)),
        PREVIEW_IMAGE,
    )?;
    ensure_subject(&image, "surface-mesh")?;
    write_png(&output.join("surface-mesh.png"), &image)
}

fn audit_transitions(root: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let path = root.join("benchmarks/scenes/3PTB.cif");
    let Some(path) = path.to_str() else {
        return Err(io::Error::other("transition input path is not valid UTF-8").into());
    };
    let (mut app, _) = scenes::build_plain_scene(path).map_err(io::Error::other)?;
    let camera = camera(&app, image_aspect(PREVIEW_IMAGE));
    let mut engine = Engine::new(&audit_engine(), None)?;
    for choice in catalog::choices().iter().copied() {
        if !app.set_representation(choice) {
            println!(
                "{:<18} skipped for 3PTB (incompatible molecular topology)",
                choice.name()
            );
            continue;
        }
        let image = engine.render_image(&app.scene, &camera, PREVIEW_IMAGE)?;
        ensure_subject(&image, &format!("transition-{}", choice.name()))?;
        let destination = output.join(format!("transition-{}.png", choice.name()));
        write_png(&destination, &image)?;
    }
    Ok(())
}

fn source_for(choice: catalog::RepresentationChoice) -> &'static str {
    match choice {
        catalog::RepresentationChoice::Kind(pdviewx::RepresentationKind::PaperChain) => {
            GLYCAN_SOURCE
        }
        _ => "3PTB.cif",
    }
}

fn audit_app(root: &Path, choice: catalog::RepresentationChoice) -> Result<App, Box<dyn Error>> {
    if choice == catalog::RepresentationChoice::Kind(RepresentationKind::Twister) {
        return twister_app(root);
    }
    let path = root.join("benchmarks/scenes").join(source_for(choice));
    let Some(path) = path.to_str() else {
        return Err(io::Error::other("audit input path is not valid UTF-8").into());
    };
    scenes::build_plain_scene(path)
        .map(|(app, _)| app)
        .map_err(|error| io::Error::other(format!("{}: {error}", choice.name())).into())
}

/// Frames one deposited glycan site drawn as a twister ribbon.
///
/// The sugars come from a real entry rather than from generated coordinates
/// because the whole point of this representation is the angle between one ring
/// plane and the next. Ideal rings placed by hand share an orientation, so they
/// would produce a flat plate and audit nothing; deposited sugars carry the
/// pucker and the glycosidic torsion the ribbon is supposed to report.
fn twister_app(root: &Path) -> Result<App, Box<dyn Error>> {
    let path = root.join("benchmarks/scenes").join(GLYCAN_SOURCE);
    let Some(path) = path.to_str() else {
        return Err(io::Error::other("glycan audit path is not valid UTF-8").into());
    };
    let structure = common::read_structure(path).map_err(io::Error::other)?;
    let site = glycan_site(&structure).ok_or_else(|| {
        io::Error::other(format!(
            "{GLYCAN_SOURCE} carries no linked {GLYCAN_COMPONENT} site"
        ))
    })?;
    let mut scene = Scene::new();
    let owner = scene.add_structure(&structure)?;
    let selection = scene.add_structure_selection(owner, AtomSelection::Sparse(site.atoms))?;
    let representation = scene.represent(selection, RepresentationKind::Twister)?;
    if let Some(value) = scene.representation_mut(representation) {
        value.color = ColorScheme::ByChain;
    }
    Ok(App {
        scene,
        representations: vec![representation],
        representation_index: 0,
        surface_style_index: 0,
        putty_domain: None,
        color_index: 1,
        available_choices: [true; 14],
        bound: Aabb::from_points(site.points.iter().copied()).bounding_sphere(),
        state: None,
        holo: None,
    })
}

/// Atoms and positions of the first chain holding a linked run of sugars.
struct GlycanSite {
    atoms: Vec<u32>,
    points: Vec<Vec3>,
}

/// Finds one glycan site: a chain carrying at least two sugar residues.
///
/// Deposited glycans are modelled as their own short chains, so a chain is the
/// natural unit here and the first qualifying one is taken deterministically by
/// file order rather than chosen by name.
fn glycan_site(structure: &pdbiox::Structure) -> Option<GlycanSite> {
    for chain in structure.data().chains() {
        let sugars = chain
            .residues()
            .filter(|residue| residue.name() == Some(GLYCAN_COMPONENT));
        let mut atoms = Vec::new();
        let mut points = Vec::new();
        let mut count = 0usize;
        for residue in sugars {
            count += 1;
            for atom in residue.atoms() {
                atoms.push(atom.index().get());
                if let Some(position) = atom.position() {
                    points.push(Vec3::from(position));
                }
            }
        }
        if count >= MIN_GLYCAN_SUGARS && !points.is_empty() {
            atoms.sort_unstable();
            return Some(GlycanSite { atoms, points });
        }
    }
    None
}

fn image_aspect(config: ImageConfig) -> f32 {
    // Render targets are bounded well below the point where a float stops
    // counting pixels exactly, so a saturating narrow is exact here and the
    // clamp only guards a zero-height target.
    let width = f32::from(match u16::try_from(config.width) {
        Ok(width) => width,
        Err(_) => u16::MAX,
    });
    let height = f32::from(match u16::try_from(config.height) {
        Ok(height) => height,
        Err(_) => u16::MAX,
    });
    width / height.max(1.0)
}

fn camera(app: &App, aspect: f32) -> Camera {
    Camera::framing(&app.bound, aspect)
}

fn camera_for(app: &App, choice: catalog::RepresentationChoice, aspect: f32) -> Camera {
    if matches!(
        choice,
        catalog::RepresentationChoice::Kind(
            RepresentationKind::Twister | RepresentationKind::PaperChain
        )
    ) {
        return paper_chain_camera(&app.scene.world_aabb(), aspect);
    }
    camera(app, aspect)
}

fn paper_chain_camera(bound: &Aabb, aspect: f32) -> Camera {
    if bound.is_empty() {
        return Camera::framing_aabb(bound, aspect);
    }
    let extents = bound.half_extents().to_array();
    let mut long_axis = 0usize;
    for axis in 1..3 {
        if extents[axis] > extents[long_axis] {
            long_axis = axis;
        }
    }
    let mut depth_axis = usize::from(long_axis == 0);
    for axis in 0..3 {
        if axis != long_axis && extents[axis] < extents[depth_axis] {
            depth_axis = axis;
        }
    }
    let cross_axis = 3usize.saturating_sub(long_axis + depth_axis);
    let sphere = bound.bounding_sphere();
    let mut camera = Camera::framing(&sphere, aspect);
    let distance = camera.focus_distance() * 0.72;
    let view =
        (axis_vector(depth_axis) + axis_vector(cross_axis) * 0.45 + axis_vector(long_axis) * 0.16)
            .normalize();
    let long = axis_vector(long_axis);
    let up = (long - view * long.dot(view)).normalize();
    camera.eye = sphere.center + view * distance;
    camera.target = sphere.center;
    camera.up = up;
    camera.projection.fit_near_far(camera.eye, &sphere);
    camera
}

const fn axis_vector(axis: usize) -> Vec3 {
    match axis {
        0 => Vec3::X,
        1 => Vec3::Y,
        _ => Vec3::Z,
    }
}

fn write_png(path: &PathBuf, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

/// Rejects the all-background failure that a valid GPU submission can otherwise
/// hide. Four corners model the slight background gradient; a real molecular
/// drawing must occupy a small but material fraction of the frame.
fn ensure_subject(image: &Image, label: &str) -> Result<(), io::Error> {
    if image_has_subject(image) {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{label} rendered only the background"
    )))
}

fn image_has_subject(image: &Image) -> bool {
    let width = image.width as usize;
    let height = image.height as usize;
    if width == 0 || height == 0 || image.pixels.len() != width * height * 4 {
        return false;
    }
    let corners = [
        pixel(image, 0, 0),
        pixel(image, width - 1, 0),
        pixel(image, 0, height - 1),
        pixel(image, width - 1, height - 1),
    ];
    let required = (width * height).div_ceil(400);
    image
        .pixels
        .chunks_exact(4)
        .filter(|rgba| {
            corners.iter().all(|corner| {
                rgba[..3]
                    .iter()
                    .zip(corner)
                    .map(|(value, background)| u16::from(value.abs_diff(*background)))
                    .sum::<u16>()
                    > 24
            })
        })
        .take(required)
        .count()
        == required
}

fn pixel(image: &Image, x: usize, y: usize) -> [u8; 3] {
    let offset = (y * image.width as usize + x) * 4;
    [
        image.pixels[offset],
        image.pixels[offset + 1],
        image.pixels[offset + 2],
    ]
}

#[cfg(test)]
mod tests {
    use super::{Image, image_has_subject};

    fn image(width: u32, height: u32, rgb: [u8; 3]) -> Image {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..width * height {
            pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        Image {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn uniform_background_is_rejected() {
        assert!(!image_has_subject(&image(20, 20, [245, 245, 246])));
    }

    #[test]
    fn material_foreground_is_accepted() {
        let mut value = image(20, 20, [245, 245, 246]);
        for pixel in value.pixels.chunks_exact_mut(4).skip(100).take(10) {
            pixel[..3].copy_from_slice(&[80, 120, 200]);
        }
        assert!(image_has_subject(&value));
    }
}

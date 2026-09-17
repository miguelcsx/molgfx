//! A structure inside the crystallographic cell its file declares.
//!
//! The cell is drawn as twelve ordinary guides. Nothing about it is inferred by
//! the engine: the caller reads `_cell` from the entry, converts the lengths and
//! angles into the three cell vectors, and hands over plain geometry — the
//! division the specification asks for, and the reason a decorative box can
//! never be mistaken for detected crystallography.
//!
//! Usage: `cargo run --example unit_cell --release --features semantic -- [structure.cif] [out.png]`

use molgfx::{
    AtomSelection, BoundingSphere, Camera, ColorScheme, Engine, EngineConfig, Guide, GuideStyle,
    Image, ImageConfig, Material, Mesh, MeshVertex, RelationPattern, RenderMode, RenderProfile,
    RepresentationKind, Rgba8, Scene, StructureHandle, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::path::Path;

#[path = "common/mod.rs"]
mod common;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let path = arguments
        .first()
        .map_or("benchmarks/scenes/3PTB.cif", String::as_str);
    let output = arguments.get(1).map_or("unit_cell.png", String::as_str);

    let structure = common::read_structure(path)?;
    let mut scene = Scene::from_structure(&structure)?;
    let Some((owner, _)) = scene.structures().next() else {
        return Err("structure did not place".into());
    };
    let records = common::deposited_secondary_structure(path, &structure);
    if !records.is_empty() {
        scene.apply_secondary_structure(owner, &records)?;
    }

    let selection = scene.add_selection(AtomSelection::All);
    let represented = scene.represent(selection, RepresentationKind::Cartoon)?;
    let Some(representation) = scene.representation_mut(represented) else {
        return Err("new representation became stale".into());
    };
    representation.color = ColorScheme::BySecondaryStructure;

    // Dashed so the cell reads as an annotation over the molecule rather than
    // as another piece of matter in the scene.
    let style = GuideStyle {
        color: Rgba8::opaque(90, 100, 118),
        pattern: RelationPattern::Dashed,
        width_pixels: 1.4,
        opacity: 0.85,
        period_pixels: 11.0,
        duty_cycle: 0.55,
        ..GuideStyle::default()
    };
    let edges = unit_cell_guides(path, owner, style);
    let cell_edges = edges.len();
    for guide in edges {
        scene.add_guide(guide)?;
    }

    // A caller mesh: the ground plane the cell sits on. Nothing derives it
    // from the structure — it is plain triangles handed to the engine.
    let floor = ground_plane(owner, &scene);
    if let Ok(mesh) = floor {
        scene.add_mesh(mesh)?;
    }

    let config = ImageConfig {
        width: 1400,
        height: 1100,
    };
    let bound = scene.world_aabb().bounding_sphere();
    let mut camera = Camera::framing(
        &BoundingSphere {
            center: bound.center,
            radius: bound.radius * 1.05,
        },
        1400.0 / 1100.0,
    );
    camera.projection.fit_near_far(camera.eye, &bound);

    let mut engine = Engine::new(
        &EngineConfig {
            mode: RenderMode::Cinematic,
            profile: RenderProfile::illustrative(),
            ..EngineConfig::default()
        },
        None,
    )?;
    let image = engine.render_image(&scene, &camera, config)?;
    write_png(output, &image)?;
    println!("drew {cell_edges} cell edges around {path} into {output}");
    Ok(())
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

/// The twelve edges of the crystallographic unit cell an mmCIF declares.
///
/// The engine draws guides; it does not read crystallography. So the cell
/// parameters are parsed here, converted to the three cell vectors by the
/// standard construction, and handed over as ordinary caller geometry — which
/// is exactly the division the specification asks for.
///
/// Returns an empty vector when the file declares no cell.
#[must_use]
pub fn unit_cell_guides(path: &str, owner: StructureHandle, style: GuideStyle) -> Vec<Guide> {
    let Some([a, b, c, alpha, beta, gamma]) = cell_parameters(path) else {
        return Vec::new();
    };
    let (alpha, beta, gamma) = (alpha.to_radians(), beta.to_radians(), gamma.to_radians());
    // The conventional orientation: a along x, b in the xy plane, c completing
    // the right-handed cell.
    let axis_a = Vec3::new(a, 0.0, 0.0);
    let axis_b = Vec3::new(b * gamma.cos(), b * gamma.sin(), 0.0);
    let cx = beta.cos();
    let cy = (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin().max(1.0e-6);
    let cz_squared = 1.0 - cx * cx - cy * cy;
    if !cz_squared.is_finite() || cz_squared <= 0.0 {
        return Vec::new();
    }
    let axis_c = Vec3::new(c * cx, c * cy, c * cz_squared.sqrt());

    let corner = |i: u32| {
        let pick = |bit: u32, axis: Vec3| if i & bit == 0 { Vec3::ZERO } else { axis };
        pick(1, axis_a) + pick(2, axis_b) + pick(4, axis_c)
    };
    // Corners differing in exactly one bit share an edge.
    let mut guides = Vec::with_capacity(12);
    for start in 0..8u32 {
        for bit in [1u32, 2, 4] {
            let end = start | bit;
            if end == start {
                continue;
            }
            if let Ok(guide) = Guide::new(owner, corner(start), corner(end), style) {
                guides.push(guide);
            }
        }
    }
    guides
}

/// Reads `_cell` lengths in Ångström and angles in degrees.
fn cell_parameters(path: &str) -> Option<[f32; 6]> {
    let bytes = std::fs::read(path).ok()?;
    let input = pdbiox::InputBuffer::from_bytes(bytes);
    let (document, _diagnostics) = pdbiox::cif::parse(&input).ok()?;
    let block = document.first_block()?;
    let cell = block.category("cell")?;
    let rows = pdbiox::cif::Rows::new(cell);
    // Cell parameters are written with a few decimals; parsing the text
    // straight into `f32` avoids a lossy narrowing from the reader's `f64`.
    let read = |item: &str| {
        rows.identifier(item)
            .and_then(|value| value.parse::<f32>().ok())
    };
    Some([
        read("length_a")?,
        read("length_b")?,
        read("length_c")?,
        read("angle_alpha")?,
        read("angle_beta")?,
        read("angle_gamma")?,
    ])
}

/// A shaded quad under the cell, as two caller-supplied triangles.
fn ground_plane(owner: StructureHandle, scene: &Scene) -> Result<Mesh, molgfx::CoreError> {
    let bounds = scene.world_aabb();
    let centre = bounds.center();
    let reach = bounds.half_extents().max_element() * 1.6;
    let base = bounds.min.y - reach * 0.05;
    let vertex = |x: f32, z: f32| MeshVertex {
        position: Vec3::new(x, base, z),
        normal: Vec3::Y,
        color: Rgba8::opaque(196, 202, 210),
    };
    let vertices = vec![
        vertex(centre.x - reach, centre.z - reach),
        vertex(centre.x + reach, centre.z - reach),
        vertex(centre.x + reach, centre.z + reach),
        vertex(centre.x - reach, centre.z + reach),
    ];
    Mesh::new(owner, vertices, vec![0, 1, 2, 0, 2, 3], Material::default())
}

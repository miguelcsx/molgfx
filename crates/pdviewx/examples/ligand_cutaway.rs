//! Renders one named ligand inside a clipped molecular surface.
//!
//! Usage: `cargo run --example ligand_cutaway --release -- structure.cif LIG output.png [inspection|illustrative|cinematic] [profile]`

use pdviewx::{
    Aabb, AtomSelection, BoundingSphere, Camera, ClipPlane, ClipSet, ColorScheme, Engine,
    EngineConfig, FocusScene, FocusStyle, FocusSurfaceExtent, Image, ImageConfig, RenderProfile,
    RepresentationKind, Rgba8, Scene, SurfaceStyle, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

struct LigandSelections {
    ligand: Vec<u32>,
    ligand_points: Vec<Vec3>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let path = required(&arguments, 0, "a structure path")?;
    let ligand_name = required(&arguments, 1, "a ligand component name")?;
    let output = required(&arguments, 2, "an output PNG path")?;
    // The profile's own backdrop is used as-is: a dark studio sweep behind a
    // pocket reads the way a documentary lights a specimen, and overriding it
    // here would only fight the recipe the caller asked for.
    let profile = render_profile(arguments.get(3).map(String::as_str))?;
    let should_profile = arguments.get(4).is_some_and(|value| value == "profile");
    let parsed = pdbiox::read(path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let structure = pdbiox::infer_bonds(&parsed, pdbiox::BondInference::default())
        .map_err(|diagnostic| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bond inference diagnostic: {diagnostic:?}"),
            )
        })?
        .structure;
    let LigandSelections {
        ligand,
        ligand_points,
    } = selections(&structure, ligand_name)?;
    let ligand_bound = Aabb::from_points(ligand_points.iter().copied());
    let view_direction = ligand_view_direction(&ligand_points, ligand_bound.center());
    let mut scene = Scene::from_structure(&structure)?;
    let ligand = scene.add_selection(AtomSelection::Sparse(ligand));
    let focus = scene.focus_with(
        ligand,
        FocusStyle {
            context_opacity: 0.34,
            context_color: Rgba8::opaque(74, 78, 82),
            solvent_opacity: 0.0,
            surface_extent: FocusSurfaceExtent::Pocket,
            ..FocusStyle::default()
        },
    )?;

    let cut = ClipPlane::from_point_normal(ligand_bound.center(), -view_direction)?;
    let Some(surface) = scene.representation_mut(focus.pocket_representation) else {
        return Err(io::Error::other("surface handle became stale").into());
    };
    surface.clipping = ClipSet::new(&[cut])?;
    surface.params.surface_style = SurfaceStyle::Solid;
    surface.color = ColorScheme::Uniform(Rgba8::opaque(48, 68, 67));
    surface.material.opacity = 1.0;
    surface.material.roughness = 0.74;
    surface.material.specular = 0.14;

    let Some(neighbourhood) = scene.representation_mut(focus.near_representation) else {
        return Err(io::Error::other("neighbourhood handle became stale").into());
    };
    neighbourhood.kind = RepresentationKind::Lines;
    neighbourhood.params.line_width_pixels = 1.10;
    neighbourhood.material.opacity = 0.72;

    let Some(ligand_representation) = scene.representation_mut(focus.focus_representation) else {
        return Err(io::Error::other("ligand handle became stale").into());
    };
    ligand_representation.params.radius_scale = 0.34;
    ligand_representation.params.bond_radius = 0.20;
    ligand_representation.material.roughness = 0.38;
    ligand_representation.material.specular = 0.34;
    ligand_representation.order = 1;

    let config = ImageConfig {
        width: 1280,
        height: 960,
    };
    let ligand_sphere = BoundingSphere::from_points(&ligand_points);
    let sphere = BoundingSphere {
        center: ligand_sphere.center,
        radius: ligand_sphere.radius * 1.55,
    };
    let mut camera = Camera::framing(&sphere, 4.0 / 3.0);
    let distance = camera.eye.distance(camera.target);
    camera.eye = camera.target + view_direction * distance;
    camera.up = ligand_up_direction(&ligand_points, camera.target, view_direction);
    camera
        .projection
        .fit_near_far(camera.eye, &scene.world_aabb().bounding_sphere());
    let engine_config = EngineConfig {
        profile,
        ..EngineConfig::default()
    };
    let mut engine = Engine::new(&engine_config, None)?;
    let image = engine.render_image(&scene, &camera, config)?;
    write_png(output, &image)?;
    if should_profile {
        profile_frames(&mut engine, &scene, &camera, config)?;
    }
    println!("rendered {ligand_name} inside a clipped local SES pocket");
    Ok(())
}

fn profile_frames(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    config: ImageConfig,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..20 {
        engine.profile_frame(scene, camera, config)?;
    }
    let mut gpu = Vec::with_capacity(120);
    let mut cpu = Vec::with_capacity(120);
    for _ in 0..120 {
        let timing = engine.profile_frame(scene, camera, config)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    let median = |values: &[u64]| values.get(values.len() / 2).copied();
    let p99 = |values: &[u64]| {
        values
            .get((values.len() * 99).div_ceil(100).saturating_sub(1))
            .copied()
    };
    let (Some(gpu_median), Some(gpu_p99), Some(cpu_median)) =
        (median(&gpu), p99(&gpu), median(&cpu))
    else {
        return Err(io::Error::other("profiling produced no samples").into());
    };
    println!(
        "ligand pocket: GPU median {gpu_median} ns ({:.2} FPS), GPU p99 {gpu_p99} ns ({:.2} FPS), CPU median {cpu_median} ns",
        1.0 / std::time::Duration::from_nanos(gpu_median).as_secs_f64(),
        1.0 / std::time::Duration::from_nanos(gpu_p99).as_secs_f64(),
    );
    Ok(())
}

fn render_profile(name: Option<&str>) -> Result<RenderProfile, io::Error> {
    match name {
        None | Some("illustrative") => Ok(RenderProfile::illustrative()),
        Some("inspection") => Ok(RenderProfile::inspection()),
        Some("cinematic") => Ok(RenderProfile::cinematic()),
        Some(name) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown render profile {name}"),
        )),
    }
}

fn required<'a>(
    values: &'a [String],
    index: usize,
    description: &str,
) -> Result<&'a str, io::Error> {
    values.get(index).map(String::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing {description}"),
        )
    })
}

fn selections(
    structure: &pdbiox::Structure,
    ligand_name: &str,
) -> Result<LigandSelections, io::Error> {
    let data = structure.data();
    let Some(ligand_residue) = data
        .residues()
        .find(|residue| residue.name() == Some(ligand_name))
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("component {ligand_name} is absent"),
        ));
    };
    let ligand = ligand_residue
        .atoms()
        .map(|atom| atom.index().get())
        .collect::<Vec<_>>();
    let ligand_points = ligand
        .iter()
        .filter_map(|index| {
            structure
                .positions()
                .get(usize::try_from(*index).ok()?)
                .copied()
                .map(Vec3::from_array)
        })
        .collect::<Vec<_>>();
    if ligand.is_empty() || ligand_points.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protein or ligand selection has no positioned atoms",
        ));
    }
    Ok(LigandSelections {
        ligand,
        ligand_points,
    })
}

fn ligand_view_direction(points: &[Vec3], center: Vec3) -> Vec3 {
    let primary = points
        .iter()
        .map(|point| *point - center)
        .max_by(|left, right| left.length_squared().total_cmp(&right.length_squared()))
        .map_or(Vec3::X, |direction| direction);
    let secondary = points
        .iter()
        .map(|point| *point - center)
        .max_by(|left, right| {
            primary
                .cross(*left)
                .length_squared()
                .total_cmp(&primary.cross(*right).length_squared())
        })
        .map_or(Vec3::Y, |direction| direction);
    let mut normal = primary.cross(secondary).normalize_or_zero();
    if normal.length_squared() < 0.5 {
        normal = Vec3::Z;
    }
    if normal.dot(Vec3::Z) < 0.0 {
        -normal
    } else {
        normal
    }
}

fn ligand_up_direction(points: &[Vec3], center: Vec3, view: Vec3) -> Vec3 {
    points
        .iter()
        .map(|point| (*point - center).reject_from_normalized(view))
        .max_by(|left, right| left.length_squared().total_cmp(&right.length_squared()))
        .map_or(Vec3::Y, Vec3::normalize_or_zero)
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

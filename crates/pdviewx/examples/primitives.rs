//! Renders caller-authored generic particles, analytic glyphs and
//! a caller-integrated streamline in one disposable feature fixture.
//!
//! The example deliberately does not solve a vector field or infer chemistry.
//! It exercises the renderer-side envelope that can receive those results from
//! an upstream provider or application, which is the honest comparison point
//! for VMD `FieldLines` and OVITO particle/vector/line objects.

use pdviewx::{
    AnisotropicEllipsoid, Camera, CarbohydrateShape, CarbohydrateSymbol, Engine, EngineConfig,
    GuideCap, GuideStyle, Image, ImageConfig, InteractionPattern, Particle, ParticleBoundary,
    ParticleMotion, ParticleShape, PlanarRegion, Rgba8, Scene, Vec3,
};
use pdviewx_math::Quat;
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const IMAGE: ImageConfig = ImageConfig {
    width: 1024,
    height: 768,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/primitives.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::from_structure(&structure)?;
    let Some((owner, _)) = scene.structures().next() else {
        return Err(io::Error::other("fixture structure is absent").into());
    };
    let bounds = scene.world_aabb();
    let center = bounds.center();
    let half = bounds.half_extents();
    let scale = half.x.max(half.y).max(half.z).max(2.0);
    add_particles(&mut scene, owner, center, scale)?;
    add_glyphs(&mut scene, owner, center, scale)?;
    add_streamline(&mut scene, owner, center, scale)?;

    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    write_png(output, &engine.render_image(&scene, &camera, IMAGE)?)?;
    println!("wrote {output}");
    Ok(())
}

fn add_particles(
    scene: &mut Scene,
    owner: pdviewx::StructureHandle,
    center: Vec3,
    scale: f32,
) -> Result<(), Box<dyn Error>> {
    let y = center.y + scale * 1.15;
    let spacing = scale * 0.72;
    let shapes = [
        (ParticleShape::Sphere, Vec3::splat(scale * 0.24)),
        (
            ParticleShape::Box,
            Vec3::new(scale * 0.42, scale * 0.28, scale * 0.54),
        ),
        (
            ParticleShape::Cylinder,
            Vec3::new(scale * 0.34, scale * 0.34, scale * 0.72),
        ),
        (
            ParticleShape::Spherocylinder,
            Vec3::new(scale * 0.34, scale * 0.34, scale * 0.84),
        ),
        (
            ParticleShape::Gaussian,
            Vec3::new(scale * 0.56, scale * 0.42, scale * 0.32),
        ),
    ];
    let colors = [
        Rgba8::opaque(56, 189, 248),
        Rgba8::opaque(251, 191, 36),
        Rgba8::opaque(244, 114, 182),
        Rgba8::opaque(74, 222, 128),
        Rgba8::opaque(167, 139, 250),
    ];
    let offsets = [-2.0, -1.0, 0.0, 1.0, 2.0];
    for (((shape, size), color), offset) in shapes.into_iter().zip(colors).zip(offsets) {
        let motion = if shape == ParticleShape::Gaussian {
            Some(
                ParticleMotion::new(
                    Vec3::new(0.18, 0.0, 0.0),
                    scene.world_aabb(),
                    0.016,
                    23,
                    ParticleBoundary::Wrap,
                )?
                .with_respawn_after_steps(48),
            )
        } else {
            None
        };
        let particle = Particle::new(
            owner,
            center + Vec3::new(offset * spacing, y, 0.0),
            Quat::from_rotation_z(offset * 0.12),
            size,
            shape,
            color,
            if shape == ParticleShape::Gaussian {
                0.72
            } else {
                0.96
            },
        )?;
        let particle = match motion {
            Some(motion) => particle.with_motion(motion),
            None => particle,
        };
        scene.add_particle(particle)?;
    }
    Ok(())
}

fn add_glyphs(
    scene: &mut Scene,
    owner: pdviewx::StructureHandle,
    center: Vec3,
    scale: f32,
) -> Result<(), Box<dyn Error>> {
    let ellipsoid = AnisotropicEllipsoid::new(
        center + Vec3::new(-scale * 0.72, -scale * 1.12, 0.0),
        [scale * 0.38, scale * 0.16, scale * 0.24, 0.0, 0.0, 0.0],
    )?;
    scene.add_ellipsoid(owner, ellipsoid, Rgba8::opaque(248, 113, 113), 0.9)?;

    let symbol = CarbohydrateSymbol::new(
        owner,
        center + Vec3::new(0.0, -scale * 1.12, 0.0),
        Quat::from_rotation_z(0.25),
        Vec3::splat(scale * 0.46),
        CarbohydrateShape::Glc,
        Rgba8::opaque(251, 146, 60),
    )?;
    scene.add_carbohydrate_symbol(symbol)?;

    let plane = PlanarRegion::new(
        owner,
        center + Vec3::new(scale * 0.72, -scale * 1.12, 0.0),
        Vec3::Z,
        Vec3::X,
        [scale * 0.72, scale * 0.36],
    )?;
    scene.add_filled_planar_region(plane, Rgba8::opaque(45, 212, 191), 0.46)?;
    Ok(())
}

fn add_streamline(
    scene: &mut Scene,
    owner: pdviewx::StructureHandle,
    center: Vec3,
    scale: f32,
) -> Result<(), Box<dyn Error>> {
    let points = (0_u16..=16)
        .map(|index| {
            let t = f32::from(index) / 16.0;
            let x = center.x + (t - 0.5) * scale * 3.6;
            let y = center.y - scale * 1.62 + (t * std::f32::consts::TAU).sin() * scale * 0.16;
            Vec3::new(x, y, center.z)
        })
        .collect::<Vec<_>>();
    let style = GuideStyle {
        color: Rgba8::opaque(226, 232, 240),
        pattern: InteractionPattern::Dashes,
        width_pixels: 2.4,
        opacity: 0.92,
        cap: GuideCap::Arrow,
        ..GuideStyle::default()
    };
    scene.add_streamline(owner, &points, style)?;
    Ok(())
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

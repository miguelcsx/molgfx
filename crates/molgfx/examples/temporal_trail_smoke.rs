//! Visual audit for continuous, history-bounded screen-space motion trails.

use molgfx::{
    Aabb, BackdropStyle, BoundingSphere, Camera, EffectLayer, Engine, EngineConfig,
    IllustrationStyle, ImageConfig, Particle, ParticleBoundary, ParticleMotion, ParticleShape,
    PresentationEffect, Primitive, Quat, RenderProfile, Rgba8, Scene, SequenceConfig, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};

const FRAME_COUNT: u16 = 90;

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args().nth(1).map_or_else(
        || PathBuf::from("/tmp/molgfx-temporal-trails"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&output)?;
    let structure = read_structure("benchmarks/scenes/1ubq.cif")?;
    let mut scene = Scene::from_structure(&structure)?;
    let Some((owner, _)) = scene.structures().next() else {
        return Err(std::io::Error::other("trail scene has no owner").into());
    };
    let bounds = Aabb::new(Vec3::new(-6.0, -3.0, -1.0), Vec3::new(6.0, 3.0, 1.0));
    let particles = [
        moving_particle(owner, -1.4, 5.0, Rgba8::opaque(255, 92, 92), bounds, 7)?,
        moving_particle(owner, 0.0, 5.5, Rgba8::opaque(82, 190, 255), bounds, 11)?,
        moving_particle(owner, 1.4, 6.0, Rgba8::opaque(255, 206, 78), bounds, 17)?,
    ];
    scene.add_primitives(&particles.map(Primitive::particle))?;
    let camera = Camera::framing(
        &BoundingSphere {
            center: Vec3::ZERO,
            radius: 7.0,
        },
        4.0 / 3.0,
    );
    let profile = RenderProfile::inspection()
        .with_layer(EffectLayer::new(PresentationEffect::Backdrop(
            BackdropStyle {
                top: Rgba8::opaque(20, 28, 44),
                bottom: Rgba8::opaque(5, 8, 15),
                glow_color: Rgba8::opaque(18, 40, 62),
                glow_strength: 0.08,
            },
        )))
        .with_layer(EffectLayer::new(PresentationEffect::Illustration(
            IllustrationStyle {
                motion_persistence: 0.72,
                ..IllustrationStyle::default()
            },
        )));
    let mut engine = Engine::new(
        &EngineConfig {
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    let config = ImageConfig {
        width: 800,
        height: 600,
    };
    let sequence_config = SequenceConfig::at_fps(config, 60, 3)?;
    let mut sequence = engine.sequence(sequence_config)?;
    for frame in 0..FRAME_COUNT {
        sequence.submit(&mut engine, &scene, &camera, u64::from(frame))?;
        if sequence.pending() == usize::from(sequence_config.max_in_flight) {
            for completed in sequence.finish(&mut engine)? {
                write_png(
                    output.join(format!("trail-{:03}.png", completed.ticket.timestamp)),
                    &completed.image,
                )?;
            }
            sequence = engine.sequence(sequence_config)?;
        }
    }
    for completed in sequence.finish(&mut engine)? {
        write_png(
            output.join(format!("trail-{:03}.png", completed.ticket.timestamp)),
            &completed.image,
        )?;
    }
    println!(
        "wrote {FRAME_COUNT} temporal-trail frames to {}",
        output.display()
    );
    Ok(())
}

fn moving_particle(
    owner: molgfx::StructureHandle,
    y: f32,
    speed: f32,
    color: Rgba8,
    bounds: Aabb,
    random_seed: u32,
) -> Result<Particle, molgfx::CoreError> {
    let motion = ParticleMotion::new(
        Vec3::new(speed, speed * 0.025, 0.0),
        bounds,
        1.0 / 60.0,
        random_seed,
        ParticleBoundary::Wrap,
    )?;
    Particle::new(
        owner,
        Vec3::new(-5.5, y, 0.0),
        Quat::IDENTITY,
        Vec3::splat(1.1),
        ParticleShape::Sphere,
        color,
        1.0,
    )
    .map(|particle| particle.with_motion(motion))
}

fn write_png(path: impl AsRef<Path>, image: &molgfx::Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

fn read_structure(path: &str) -> Result<pdbiox::Structure, Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    pdbiox::read_bytes(bytes, Some(path), &pdbiox::ReadOptions::new()).map_or_else(
        |diagnostics| Err(format!("trail fixture parses: {diagnostics:?}").into()),
        |(structure, _)| Ok(structure),
    )
}

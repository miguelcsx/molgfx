//! Script cases: scenes built from command text, fitted cameras and orbit frames.
use super::{
    catalog::{Camera, Catalog, FitCamera, Fixture, Result, Video, VideoKind},
    native,
};
use molgfx::{
    Renderer, Scene, StructureId, TrajectoryBinding, TrajectoryFrame, camera, command::Session,
    profile::MeasuredOutput, schema::DataSource, trajectory,
};
use molgfx_bench::fallback;
use num_traits::ToPrimitive;
use serde_json::Value;
use std::{io, path::Path};

/// Orbit keyframes per revolution; linear easing between them is a close polygon of the circle.
const ORBIT_KEYFRAMES: u32 = 12;

/// Builds the scene of a script case by executing its command lines in order.
pub(super) fn scene(fixture: &Fixture, cache: &Path) -> Result<(Scene, Value)> {
    let script = fixture
        .script
        .as_ref()
        .ok_or_else(|| io::Error::other("script scene requested for a non-script case"))?;
    if fixture.format == "mrc" {
        return Err(io::Error::other("prerequisite: volume-only scene").into());
    }
    let (structure, metadata) = native::structure(fixture, cache)?;
    let mut scene = Scene::from_structure(&structure)?;
    let mut session = Session::new(&scene);
    for line in &script.molgfx {
        session
            .execute_text(&mut scene, line)
            .map_err(|errors| io::Error::other(format!("{line:?}: {}", errors.render(line))))?;
    }
    Ok((scene, metadata))
}

/// Resolves the camera fitted to `fit.selection`: the bounding sphere, looking down -Z with +Y up.
pub(super) fn fit(fixture: &Fixture, cache: &Path, fit: &FitCamera) -> Result<Camera> {
    let (scene, _) = scene(fixture, cache)?;
    let bounds = scene
        .selection_bounds(fit.selection.as_str())?
        .ok_or_else(|| io::Error::other("fit selection matches no atoms"))?;
    let sphere = bounds.bounding_sphere();
    let radius = sphere.radius * fit.margin;
    let distance = radius / (fit.fov_y_degrees.to_radians() * 0.5).sin();
    let target = sphere.center.to_array();
    Ok(Camera {
        position: [target[0], target[1], target[2] + distance],
        target,
        up: [0.0, 1.0, 0.0],
        fov_y_degrees: fit.fov_y_degrees,
        near: (distance - radius).max(0.1),
        far: distance + radius,
    })
}

/// Renders the case's video frames into `directory/frames/%04d.png`.
pub(super) fn frames(
    catalog: &Catalog,
    fixture: &Fixture,
    cache: &Path,
    scene: &Scene,
    renderer: &mut Renderer,
    video: &Video,
    directory: &Path,
) -> Result<()> {
    if let VideoKind::Trajectory = video.kind {
        return trajectory(catalog, fixture, cache, renderer, video, directory);
    }
    let size = (catalog.extent[0], catalog.extent[1]);
    let aspect = (f64::from(size.0) / f64::from(size.1))
        .to_f32()
        .ok_or_else(|| io::Error::other("aspect ratio is not representable"))?;
    let c = &fixture.camera;
    let offset = [
        c.position[0] - c.target[0],
        c.position[1] - c.target[1],
        c.position[2] - c.target[2],
    ];
    let seconds = f64::from(video.frames.saturating_sub(1)) / f64::from(video.fps);
    let mut keyframes = Vec::new();
    for step in 0..=ORBIT_KEYFRAMES {
        let turn = f64::from(step) / f64::from(ORBIT_KEYFRAMES);
        let angle = (turn * std::f64::consts::TAU)
            .to_f32()
            .ok_or_else(|| io::Error::other("orbit angle is not representable"))?;
        let (sin, cos) = angle.sin_cos();
        let position = [
            c.target[0] + offset[0] * cos + offset[2] * sin,
            c.target[1] + offset[1],
            c.target[2] - offset[0] * sin + offset[2] * cos,
        ];
        keyframes.push((
            turn * seconds,
            camera::perspective(
                position,
                c.target,
                c.up,
                c.fov_y_degrees.to_radians(),
                aspect,
                c.near,
                c.far,
            )?,
        ));
    }
    let path = camera::path(&keyframes, camera::CameraEasing::Linear)?;
    let images = renderer.render_camera_path(scene, &path, size, video.fps)?;
    let frames = directory.join("frames");
    std::fs::create_dir_all(&frames)?;
    for (index, image) in images.iter().enumerate() {
        image.save(frames.join(format!("{:04}.png", index + 1)))?;
    }
    Ok(())
}

/// Steps the structure's models: one interpolation interval is resident at a time.
fn trajectory(
    catalog: &Catalog,
    fixture: &Fixture,
    cache: &Path,
    renderer: &mut Renderer,
    video: &Video,
    directory: &Path,
) -> Result<()> {
    let models = molgfx_bench::reader::read_model_positions(&cache.join(&fixture.file))?;
    let Some(last) = models.len().checked_sub(1).filter(|last| *last > 0) else {
        return Err(io::Error::other(format!(
            "trajectory video needs a multi-model structure, read {} model(s)",
            models.len()
        ))
        .into());
    };
    let size = (catalog.extent[0], catalog.extent[1]);
    let camera = native::camera(catalog, fixture)?;
    let frames = directory.join("frames");
    std::fs::create_dir_all(&frames)?;
    let span = fallback(last.to_f64(), 1.0);
    let steps = f64::from(video.frames.saturating_sub(1).max(1));
    let mut resident: Option<(usize, Scene, StructureId)> = None;
    for index in 0..video.frames {
        let time = f64::from(index) / steps * span;
        let interval = fallback(time.floor().to_usize(), 0).min(last - 1);
        let sample = time
            .to_f32()
            .ok_or_else(|| io::Error::other("trajectory time is not representable"))?;
        let stale = resident.as_ref().is_none_or(|(held, ..)| *held != interval);
        if stale {
            let (mut scene, _) = scene(fixture, cache)?;
            let structure = StructureId::new(1);
            let source = DataSource::new(format!("{}:{interval}", fixture.id));
            scene.add(trajectory::trajectory(
                structure,
                source.clone(),
                fallback(models.len().to_u64(), 0),
            ))?;
            let frame = |model: usize| -> Result<TrajectoryFrame> {
                let time = model
                    .to_f32()
                    .ok_or_else(|| io::Error::other("model index is not representable"))?;
                Ok(TrajectoryFrame::new(
                    fallback(model.to_u64(), 0),
                    time,
                    std::sync::Arc::clone(&models[model]),
                ))
            };
            scene.bind_trajectory(
                TrajectoryBinding::new(source, frame(interval)?, frame(interval + 1)?)
                    .sample_seconds(sample),
            )?;
            resident = Some((interval, scene, structure));
        }
        let Some((_, scene, structure)) = resident.as_mut() else {
            continue;
        };
        scene.set_trajectory_time(*structure, sample)?;
        let image =
            renderer.render_output_with_camera(scene, &camera, size, MeasuredOutput::Converged)?;
        image.save(frames.join(format!("{:04}.png", index + 1)))?;
    }
    Ok(())
}

/// Encodes `frames/%04d.png` into `video.mp4` with the supplied ffmpeg.
pub(super) fn encode(ffmpeg: Option<&str>, video: &Video, directory: &Path) -> Result<()> {
    let ffmpeg =
        ffmpeg.ok_or_else(|| io::Error::other("a video case needs --ffmpeg PATH to encode"))?;
    let status = std::process::Command::new(ffmpeg)
        .current_dir(directory)
        .args(["-y", "-loglevel", "error", "-framerate"])
        .arg(video.fps.to_string())
        .args([
            "-i",
            "frames/%04d.png",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .args(["-crf", "16", "video.mp4"])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!("ffmpeg failed: {status}")).into());
    }
    Ok(())
}

//! Script cases: scenes built from command text, fitted cameras and orbit frames.
use super::{
    catalog::{Camera, Catalog, FitCamera, Fixture, Result, Video, VideoKind},
    native,
};
use molgfx::{Renderer, Scene, camera, command::Session};
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
    scene: &Scene,
    renderer: &mut Renderer,
    video: &Video,
    directory: &Path,
) -> Result<()> {
    let VideoKind::OrbitY = video.kind else {
        return Err(io::Error::other("prerequisite: trajectory video stepping").into());
    };
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

//! Script cases: scenes built from command text, fitted cameras and orbit frames.
use molgfx::{
    Renderer, Scene, StructureId, TrajectoryBinding, TrajectoryFrame, camera,
    schema::DataSource,
    timeline::{FrameSchedule, PlaybackMode, SequenceManifest, TimeWarp, Timeline},
    trajectory,
};
use molgfx_bench::fallback;
use molgfx_bench::gallery::{Camera, Catalog, FitCamera, Fixture, Result, Video, VideoKind};
use num_traits::ToPrimitive;
use std::{io, path::Path};

/// Orbit keyframes per revolution; linear easing between them is a close polygon of the circle.
const ORBIT_KEYFRAMES: u32 = 12;

/// Resolves the camera fitted to `fit.selection`: the bounding sphere, looking down -Z with +Y up.
pub(super) fn fit(fixture: &Fixture, cache: &Path, fit: &FitCamera) -> Result<Camera> {
    let (scene, _) = molgfx_bench::gallery::script_scene(fixture, cache)?;
    molgfx_bench::gallery::fit_camera(&scene, fit)
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
    let camera = molgfx_bench::gallery::camera(catalog.extent, &fixture.camera)?;
    let frames = directory.join("frames");
    std::fs::create_dir_all(&frames)?;
    let span = fallback(last.to_f64(), 1.0);
    let steps = f64::from(video.frames.saturating_sub(1).max(1));
    let fps = f64::from(video.fps);
    // Model time per second of playback, so frame `i` shows model `i / steps * span`.
    let rate = fps * span / steps;
    let interval_of = |index: u32| -> usize {
        let time = f64::from(index) / fps * rate;
        fallback(time.floor().to_usize(), 0).min(last - 1)
    };
    let mut manifest: Option<SequenceManifest> = None;
    let mut first = 0;
    while first < video.frames {
        // One interval is resident at a time; each run of frames inside it is
        // one deterministic timeline sequence, joined into a single manifest.
        let interval = interval_of(first);
        let mut end = first + 1;
        while end < video.frames && interval_of(end) == interval {
            end += 1;
        }
        let (mut scene, mut timeline) = interval_scene(fixture, cache, &models, interval, rate)?;
        let schedule = FrameSchedule::converged(fps, u64::from(first)..u64::from(end));
        let piece = renderer.render_timeline(
            &mut scene,
            &mut timeline,
            &camera,
            size,
            &schedule,
            |index, image| image.save(frames.join(format!("{:04}.png", index + 1))),
        )?;
        match manifest.as_mut() {
            Some(joined) => joined.append(&piece)?,
            None => manifest = Some(piece),
        }
        first = end;
    }
    if let Some(manifest) = manifest {
        manifest.write_json(directory.join("manifest.json"))?;
    }
    Ok(())
}

/// A scene holding models `interval` and `interval + 1`, with a timeline track
/// that plays them at `rate` model-seconds per playback second.
fn interval_scene(
    fixture: &Fixture,
    cache: &Path,
    models: &[std::sync::Arc<[[f32; 3]]>],
    interval: usize,
    rate: f64,
) -> Result<(Scene, Timeline)> {
    let (mut scene, _) = molgfx_bench::gallery::script_scene(fixture, cache)?;
    let structure = StructureId::new(1);
    let source = DataSource::new(format!("{}:{interval}", fixture.id));
    scene.add(trajectory::trajectory(
        structure,
        source.clone(),
        fallback(models.len().to_u64(), 0),
    ))?;
    let frame = |model: usize| -> Result<TrajectoryFrame> {
        let time = model
            .to_f64()
            .ok_or_else(|| io::Error::other("model index is not representable"))?;
        Ok(TrajectoryFrame::new(
            fallback(model.to_u64(), 0),
            time,
            std::sync::Arc::clone(&models[model]),
        ))
    };
    let start = interval
        .to_f64()
        .ok_or_else(|| io::Error::other("interval is not representable"))?;
    scene.bind_trajectory(
        TrajectoryBinding::new(source, frame(interval)?, frame(interval + 1)?)
            .sample_seconds(start),
    )?;
    // Playback origin where the clock reaches `start`; clamped to the interval.
    let warp = TimeWarp::new(
        start / rate,
        start,
        rate,
        [start, start + 1.0],
        PlaybackMode::Clamp,
    )?;
    let mut timeline = Timeline::new();
    scene.bind_timeline_trajectory(&mut timeline, structure, warp)?;
    Ok((scene, timeline))
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

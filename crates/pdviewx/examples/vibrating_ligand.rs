//! Generic deformable ligand rendered from bounded two-frame trajectory windows.

use pdviewx::{
    Aabb, AttributeColumn, AttributeValues, Camera, Engine, EngineConfig, ImageConfig,
    PlaybackMode, PointBatch, PointGlyph, PointStyle, Relation, RelationBatch, RelationPattern,
    RelationStyle, RenderMode, RenderProfile, Rgba8, RowDomain, RowEntityRef, Scene,
    SourceNamespace, SourceRows, SpatialAnchor, TimeWarp, Timeline, Vec3, VisualDescriptor,
    VisualProgramBuilder, VisualStyle,
};
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

const REALTIME_IMAGE: ImageConfig = ImageConfig {
    width: 960,
    height: 540,
};
const PUBLICATION_IMAGE: ImageConfig = ImageConfig {
    width: 3_840,
    height: 2_160,
};
const KEYFRAMES: usize = 9;
const SUBFRAMES: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args().nth(1).map_or_else(
        || PathBuf::from("target/visual-checks/vibrating-ligand"),
        PathBuf::from,
    );
    let image_config = if std::env::args().nth(2).as_deref() == Some("--4k") {
        PUBLICATION_IMAGE
    } else {
        REALTIME_IMAGE
    };
    std::fs::create_dir_all(&output)?;
    let keyframes = vibration_keyframes(&equilibrium_positions());
    let mut scene = Scene::new();
    let points = add_atoms(&mut scene, Arc::clone(&keyframes[0]))?;
    add_bonds(&mut scene, points)?;
    let camera = Camera::framing_aabb(
        &Aabb::new(Vec3::new(-3.0, -2.8, -2.0), Vec3::new(3.0, 2.8, 2.0)),
        16.0 / 9.0,
    );
    let mut engine = Engine::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::illustrative(),
            ..EngineConfig::default()
        },
        None,
    )?;
    let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)?;
    profile_animation(
        &mut engine,
        &mut scene,
        &camera,
        points,
        [&keyframes[0], &keyframes[1]],
        warp,
        image_config,
    )?;
    let mut elapsed = Vec::with_capacity((KEYFRAMES - 1) * SUBFRAMES);
    let started = Instant::now();
    let mut frame = 0_usize;
    for segment in 0..KEYFRAMES - 1 {
        let mut timeline = Timeline::new();
        let track = timeline.bind_points(
            &mut scene,
            points,
            Arc::clone(&keyframes[segment]),
            Arc::clone(&keyframes[segment + 1]),
            warp,
        )?;
        for sample in 0..SUBFRAMES {
            timeline.apply(&mut scene, sample_alpha(sample)?)?;
            let render_start = Instant::now();
            let image = engine.render_image(&scene, &camera, image_config)?;
            elapsed.push(render_start.elapsed());
            std::fs::write(
                output.join(format!("frame-{frame:03}.png")),
                image.png_bytes()?,
            )?;
            frame += 1;
        }
        timeline.unbind(&mut scene, track);
    }
    elapsed.sort_unstable();
    println!(
        "rendered {frame} frames in {:.3}s; median={:.2}ms p99={:.2}ms RSS={} MiB derived={} bytes",
        started.elapsed().as_secs_f64(),
        percentile(&elapsed, 50)?.as_secs_f64() * 1_000.0,
        percentile(&elapsed, 99)?.as_secs_f64() * 1_000.0,
        rss_mib().map_or_else(|| "unavailable".to_owned(), |value| value.to_string()),
        engine.derived_cache_usage().gpu_bytes,
    );
    println!("frames: {}", output.display());
    Ok(())
}

fn profile_animation(
    engine: &mut Engine,
    scene: &mut Scene,
    camera: &Camera,
    points: pdviewx::PointBatchHandle,
    endpoints: [&Arc<[[f32; 3]]>; 2],
    warp: TimeWarp,
    image_config: ImageConfig,
) -> Result<(), Box<dyn Error>> {
    const WARMUP: u32 = 20;
    const SAMPLES: usize = 120;
    let mut timeline = Timeline::new();
    let track = timeline.bind_points(
        scene,
        points,
        Arc::clone(endpoints[0]),
        Arc::clone(endpoints[1]),
        warp,
    )?;
    for frame in 0..WARMUP {
        timeline.apply(scene, profile_phase(frame))?;
        engine.profile_frame(scene, camera, image_config)?;
    }
    let cache_before = engine.derived_cache_usage();
    let rss_before = rss_mib();
    let mut gpu = Vec::with_capacity(SAMPLES);
    let mut cpu = Vec::with_capacity(SAMPLES);
    let mut frame_times = Vec::with_capacity(SAMPLES);
    for frame in 0..SAMPLES {
        timeline.apply(scene, profile_phase(u32::try_from(frame)? + WARMUP))?;
        let timing = engine.profile_frame(scene, camera, image_config)?;
        if timing.gpu_timing_resolved {
            gpu.push(timing.gpu_ns);
        }
        cpu.push(timing.cpu_ns);
        frame_times.push(timing.frame_ns);
    }
    timeline.unbind(scene, track);
    gpu.sort_unstable();
    cpu.sort_unstable();
    frame_times.sort_unstable();
    let cpu_p50 = percentile_u64(&cpu, 50)?;
    let cpu_p99 = percentile_u64(&cpu, 99)?;
    let frame_p50 = percentile_u64(&frame_times, 50)?;
    let frame_p99 = percentile_u64(&frame_times, 99)?;
    let cache_after = engine.derived_cache_usage();
    let gpu_summary = if gpu.is_empty() {
        "GPU timestamps unavailable".to_owned()
    } else {
        format!(
            "GPU p50={:.3}ms p99={:.3}ms",
            nanos_to_ms(percentile_u64(&gpu, 50)?),
            nanos_to_ms(percentile_u64(&gpu, 99)?),
        )
    };
    println!(
        "interactive trajectory: {gpu_summary}; CPU encode p50={:.3}ms p99={:.3}ms; end-to-end p50={:.3}ms p99={:.3}ms; RSS {} -> {} MiB; derived {} -> {} bytes",
        nanos_to_ms(cpu_p50),
        nanos_to_ms(cpu_p99),
        nanos_to_ms(frame_p50),
        nanos_to_ms(frame_p99),
        display_rss(rss_before),
        display_rss(rss_mib()),
        cache_before.gpu_bytes,
        cache_after.gpu_bytes,
    );
    Ok(())
}

fn profile_phase(frame: u32) -> f64 {
    let phase = f64::from(frame % 60) / 59.0;
    if (frame / 60).is_multiple_of(2) {
        phase
    } else {
        1.0 - phase
    }
}

fn percentile_u64(samples: &[u64], percentile: usize) -> Result<u64, io::Error> {
    let index = samples
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1);
    samples
        .get(index)
        .copied()
        .ok_or_else(|| io::Error::other("no profiling samples"))
}

fn nanos_to_ms(nanoseconds: u64) -> f64 {
    Duration::from_nanos(nanoseconds).as_secs_f64() * 1_000.0
}

fn display_rss(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |mib| mib.to_string())
}

fn add_atoms(
    scene: &mut Scene,
    positions: Arc<[[f32; 3]]>,
) -> Result<pdviewx::PointBatchHandle, Box<dyn Error>> {
    let count = u32::try_from(positions.len())?;
    let points = scene.add_point_batch(PointBatch::new(
        positions,
        SourceRows::ordered(SourceNamespace(0x0056_4942_5241_5445), count),
        PointGlyph::Sphere,
        PointStyle {
            radius: 0.34,
            color: Rgba8::WHITE,
        },
    )?);
    let domain = RowDomain::Points(points);
    let colors = scene.add_attribute(AttributeColumn::new(
        domain,
        "element color",
        AttributeValues::Color(atom_colors().into()),
    )?)?;
    let radii = scene.add_attribute(AttributeColumn::new(
        domain,
        "atom radius",
        AttributeValues::Scalar(atom_radii().into()),
    )?)?;
    let mut builder = VisualProgramBuilder::new();
    let color = builder.color_attribute(colors)?;
    let radius = builder.scalar_attribute(radii)?;
    builder.set_base_color(color)?;
    builder.set_radius_scale(radius)?;
    scene.set_domain_visual(
        domain,
        VisualDescriptor::new(VisualStyle::new(builder.finish()?)),
    )?;
    Ok(points)
}

fn add_bonds(scene: &mut Scene, points: pdviewx::PointBatchHandle) -> Result<(), Box<dyn Error>> {
    let domain = RowDomain::Points(points);
    let anchor = |row| SpatialAnchor::entity(RowEntityRef::new(domain, row));
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 0),
        (0, 6),
        (2, 7),
        (4, 8),
        (6, 9),
        (7, 10),
        (8, 11),
    ];
    let relations = pairs
        .into_iter()
        .map(|(start, end)| {
            Ok(Relation {
                start: anchor(start)?,
                end: anchor(end)?,
            })
        })
        .collect::<Result<Vec<_>, pdviewx::CoreError>>()?;
    let count = u32::try_from(relations.len())?;
    scene.add_relation_batch(RelationBatch::new(
        relations.into(),
        SourceRows::ordered(SourceNamespace(0x5649_4252_424f_4e44), count),
        RelationStyle {
            width_pixels: 4.0,
            color: Rgba8::opaque(92, 103, 119),
            opacity: 1.0,
            pattern: RelationPattern::Solid,
            endpoint_insets_pixels: [0.0, 0.0],
            depth_behind_anchors: true,
        },
    )?)?;
    Ok(())
}

fn equilibrium_positions() -> Arc<[[f32; 3]]> {
    Arc::from([
        [1.35, 0.00, 0.00],
        [0.68, 1.17, 0.04],
        [-0.68, 1.17, -0.03],
        [-1.35, 0.00, 0.02],
        [-0.68, -1.17, -0.04],
        [0.68, -1.17, 0.03],
        [2.12, 0.02, 0.05],
        [-1.08, 1.87, -0.08],
        [-1.08, -1.87, 0.08],
        [2.73, 0.08, 0.02],
        [-1.42, 2.48, -0.02],
        [-1.42, -2.48, 0.02],
    ])
}

fn vibration_keyframes(equilibrium: &[[f32; 3]]) -> Vec<Arc<[[f32; 3]]>> {
    (0..KEYFRAMES)
        .map(|frame| {
            let phase = frame_phase(frame);
            equilibrium
                .iter()
                .enumerate()
                .map(|(row, position)| {
                    let alternating = if row % 2 == 0 { 1.0 } else { -1.0 };
                    let radial = Vec3::from_array(*position).normalize_or_zero();
                    let displacement = radial * (0.055 * phase.cos())
                        + Vec3::Z * (alternating * 0.14 * phase.sin());
                    (Vec3::from_array(*position) + displacement).to_array()
                })
                .collect::<Vec<_>>()
                .into()
        })
        .collect()
}

fn frame_phase(frame: usize) -> f32 {
    const PHASES: [f32; KEYFRAMES] = [
        0.0,
        std::f32::consts::FRAC_PI_4,
        std::f32::consts::FRAC_PI_2,
        3.0 * std::f32::consts::FRAC_PI_4,
        std::f32::consts::PI,
        5.0 * std::f32::consts::FRAC_PI_4,
        3.0 * std::f32::consts::FRAC_PI_2,
        7.0 * std::f32::consts::FRAC_PI_4,
        std::f32::consts::TAU,
    ];
    PHASES.get(frame).copied().map_or(0.0, |value| value)
}

fn sample_alpha(sample: usize) -> Result<f64, io::Error> {
    let sample = u32::try_from(sample).map_err(|_| io::Error::other("sample overflow"))?;
    let count = u32::try_from(SUBFRAMES).map_err(|_| io::Error::other("sample count overflow"))?;
    Ok(f64::from(sample) / f64::from(count))
}

fn atom_colors() -> Vec<Rgba8> {
    vec![
        Rgba8::opaque(72, 86, 105),
        Rgba8::opaque(72, 86, 105),
        Rgba8::opaque(58, 117, 214),
        Rgba8::opaque(72, 86, 105),
        Rgba8::opaque(220, 66, 66),
        Rgba8::opaque(72, 86, 105),
        Rgba8::opaque(220, 66, 66),
        Rgba8::opaque(58, 117, 214),
        Rgba8::opaque(220, 66, 66),
        Rgba8::opaque(235, 235, 235),
        Rgba8::opaque(235, 235, 235),
        Rgba8::opaque(235, 235, 235),
    ]
}

fn atom_radii() -> Vec<f32> {
    vec![
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.92, 0.96, 0.92, 0.72, 0.72, 0.72,
    ]
}

fn percentile(samples: &[Duration], percentile: usize) -> Result<Duration, io::Error> {
    let index = samples
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1);
    samples
        .get(index)
        .copied()
        .ok_or_else(|| io::Error::other("no render samples"))
}

fn rss_mib() -> Option<u64> {
    let pid = std::process::id().to_string();
    Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|kib| kib / 1_024)
}

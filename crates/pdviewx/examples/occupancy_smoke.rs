//! Renders a temporal ligand-residence cloud entirely from GPU accumulation.

use pdviewx::{
    AtomSelection, BackdropStyle, Camera, CameraEasing, CameraKeyframe, CameraPath, EffectLayer,
    Engine, EngineConfig, ImageConfig, OccupancyStream, PresentationEffect, RenderProfile,
    Representation, Rgba8, Scene, SequenceConfig, TrajectoryFrame, TrajectorySegment, Vec3,
    VolumeStyle, VolumeTransferFunction, VolumeTransferPoint,
};
use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FRAME_COUNT: u16 = 90;

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("/tmp/pdviewx-occupancy"), PathBuf::from);
    std::fs::create_dir_all(&output)?;
    let structure = ligand()?;
    let (mut scene, handle) = occupancy_scene(&structure)?;
    let overview = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let camera_path = audit_camera_path(overview)?;
    let profile = RenderProfile::inspection().with_layer(EffectLayer::new(
        PresentationEffect::Backdrop(BackdropStyle {
            top: Rgba8::opaque(24, 31, 48),
            bottom: Rgba8::opaque(6, 10, 18),
            glow_color: Rgba8::opaque(14, 48, 74),
            glow_strength: 0.08,
        }),
    ));
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
    let sequence_config = SequenceConfig::at_fps(config, 30, 3)?;
    let mut sequence = engine.sequence(sequence_config)?;
    for frame in 0..FRAME_COUNT {
        let time = f32::from(frame) / f32::from(FRAME_COUNT - 1);
        scene.set_trajectory_time(handle, eased(time))?;
        let camera = camera_path
            .sample(f64::from(time))
            .ok_or_else(|| std::io::Error::other("finite camera-path sample is missing"))?;
        sequence.submit(&mut engine, &scene, &camera, u64::from(frame))?;
        if sequence.pending() == usize::from(sequence_config.max_in_flight) {
            for completed in sequence.finish(&mut engine)? {
                write_png(
                    output.join(format!("occupancy-{:03}.png", completed.ticket.timestamp)),
                    &completed.image,
                )?;
            }
            sequence = engine.sequence(sequence_config)?;
        }
    }
    for completed in sequence.finish(&mut engine)? {
        write_png(
            output.join(format!("occupancy-{:03}.png", completed.ticket.timestamp)),
            &completed.image,
        )?;
    }
    println!(
        "wrote {} GPU occupancy frames to {}",
        FRAME_COUNT,
        output.display()
    );
    Ok(())
}

fn audit_camera_path(overview: Camera) -> Result<CameraPath, pdviewx::CoreError> {
    let offset = overview.eye - overview.target;
    let mut close = overview;
    close.eye = overview.target
        + Vec3::new(
            offset.z.mul_add(0.24, offset.x * 0.72),
            offset.y + 0.6,
            offset.z.mul_add(0.78, -offset.x * 0.24),
        );
    CameraPath::new(
        Arc::from([
            CameraKeyframe::new(0.0, overview)?,
            CameraKeyframe::new(1.0, close)?,
        ]),
        CameraEasing::SmoothStep,
    )
}

fn occupancy_scene(
    structure: &pdbiox::Structure,
) -> Result<(Scene, pdviewx::StructureHandle), Box<dyn Error>> {
    let mut scene = Scene::new();
    let handle = scene.add_structure(structure)?;
    let atoms = scene.add_selection(AtomSelection::All);
    scene.represent(
        atoms,
        Representation::spacefill().radius_scale(0.5).order(1),
    )?;
    scene.set_trajectory_segment(handle, trajectory()?)?;
    let stream = OccupancyStream::new(
        [96, 72, 72],
        Vec3::new(-6.4, -4.8, -4.8),
        Vec3::splat(0.133_333_34),
        0.995,
        6.0,
        200.0,
    )?;
    let volume = scene.add_occupancy_stream(handle, &AtomSelection::All, stream)?;
    let transfer = VolumeTransferFunction::new(&[
        VolumeTransferPoint::new(0.0, Rgba8::opaque(18, 38, 90), 0.0),
        VolumeTransferPoint::new(0.05, Rgba8::opaque(28, 150, 220), 0.04),
        VolumeTransferPoint::new(0.4, Rgba8::opaque(80, 226, 190), 0.24),
        VolumeTransferPoint::new(2.0, Rgba8::opaque(255, 198, 72), 0.72),
    ])?;
    scene.represent(
        volume,
        Representation::volume().volume_style(
            VolumeStyle::default()
                .sampling(2.8, 0.45)
                .transfer(transfer),
        ),
    )?;
    Ok((scene, handle))
}

fn trajectory() -> Result<TrajectorySegment, pdviewx::CoreError> {
    let start = TrajectoryFrame::new(
        0,
        0.0,
        Arc::from([[-4.2, -1.0, 0.0], [-4.0, 0.0, 0.0], [-4.2, 1.0, 0.0]]),
        "occupancy:start",
    )?;
    let end = TrajectoryFrame::new(
        1,
        1.0,
        Arc::from([[4.2, 1.0, 0.0], [4.0, 0.0, 1.0], [4.2, -1.0, 0.0]]),
        "occupancy:end",
    )?;
    TrajectorySegment::new(start, end, 0.0)
}

fn eased(time: f32) -> f32 {
    time * time * (3.0 - 2.0 * time)
}

fn ligand() -> Result<pdbiox::Structure, Box<dyn Error>> {
    let cif = "\
data_occupancy
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
HETATM 1 C C1 . LIG A 1 1 -4.2 -1.0 0.0 1.00 10.0 1 A 1
HETATM 2 N N1 . LIG A 1 1 -4.0 0.0 0.0 1.00 10.0 1 A 1
HETATM 3 O O1 . LIG A 1 1 -4.2 1.0 0.0 1.00 10.0 1 A 1
";
    pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("occupancy-smoke.cif"),
        &pdbiox::ReadOptions::new(),
    )
    .map_or_else(
        |diagnostics| Err(format!("occupancy fixture parses: {diagnostics:?}").into()),
        |(structure, _)| Ok(structure),
    )
}

fn write_png(path: impl AsRef<Path>, image: &pdviewx::Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

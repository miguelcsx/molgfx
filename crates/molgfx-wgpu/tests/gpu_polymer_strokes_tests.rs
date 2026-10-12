//! Zero-radius polymer curves use pixel strokes over live GPU coordinates.
#![cfg(not(target_arch = "wasm32"))]
use molgfx_core::{AtomSelection, RepresentationKind, Scene, TrajectoryFrame, TrajectorySegment};
use molgfx_math::{Camera, Projection, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PickEntity, PresentationEffect, RenderMode,
    RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn source() -> molframe::Structure {
    let pdb = "ATOM      1  CA  ALA A   1      -3.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  ALA A   2       0.000   2.000   0.000  1.00 10.00           C\nATOM      3  CA  ALA A   3       3.000   0.000   0.000  1.00 10.00           C\nEND\n";
    molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some("trace.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("guides")
    .0
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_single_selected_guide_draws_half_intervals_and_picks_its_own_atom() {
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .expect("GPU");
    for kind in [
        RepresentationKind::Trace,
        RepresentationKind::Tube,
        RepresentationKind::Cartoon,
        RepresentationKind::NewRibbons,
        RepresentationKind::Ribbons,
    ] {
        for radius in [0.0, 0.25] {
            if radius == 0.0
                && matches!(
                    kind,
                    RepresentationKind::Cartoon
                        | RepresentationKind::NewRibbons
                        | RepresentationKind::Ribbons
                )
            {
                continue;
            }
            let mut scene = Scene::from_structure(&source()).expect("scene");
            let selection = scene.add_selection(AtomSelection::Range(1..2));
            let mut config = molgfx_core::RepresentationConfig::new(kind).line_width(3.0);
            if matches!(kind, RepresentationKind::Trace | RepresentationKind::Tube) {
                config = config.tube_radius(radius).expect("radius");
            }
            scene.represent(selection, config).expect("selected guide");
            let image = pixels(&mut engine, &scene, 12.0);
            let occupied = image
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .filter(|(_, pixel)| pixel[3] > 128)
                .map(|(row, _)| row)
                .collect::<Vec<_>>();
            assert!(
                occupied.len() > 40,
                "{kind:?} radius {radius}: selected guide must draw"
            );
            assert!(
                occupied.iter().all(|row| (82..=174).contains(&(row % 256))),
                "the unselected outer halves must stay hidden"
            );
            let index = *occupied.first().expect("coverage");
            let pick = engine
                .pick(
                    u32::try_from(index % 256).expect("x"),
                    u32::try_from(index / 256).expect("y"),
                )
                .expect("readback")
                .expect("polymer pick");
            assert!(matches!(pick.entity, PickEntity::Structure(identity)
                if identity.kind() == molgfx_core::EntityKind::Atom
                    && identity.row() == molgfx_core::LogicalRow::new(1)));
            assert_eq!(image, pixels(&mut engine, &scene, 12.0));
            let owner = scene.structures().next().expect("owner").0;
            let start: Arc<[[f32; 3]]> = Arc::from(source().coordinates());
            let moved: Arc<[[f32; 3]]> = start
                .iter()
                .map(|point| [point[0], point[1] + 2.0, point[2]])
                .collect();
            scene
                .set_trajectory_segment(
                    owner,
                    TrajectorySegment::new(
                        TrajectoryFrame::new(0, 0.0, start, "original").expect("start"),
                        TrajectoryFrame::new(1, 1.0, moved, "translated").expect("end"),
                        0.0,
                    )
                    .expect("interval"),
                )
                .expect("motion");
            let resident = pixels(&mut engine, &scene, 12.0);
            scene.set_trajectory_time(owner, 1.0).expect("advance");
            assert!(
                resident != pixels(&mut engine, &scene, 12.0),
                "{kind:?} follows live coordinates"
            );
            scene.set_trajectory_time(owner, 0.0).expect("reverse");
            assert!(
                resident == pixels(&mut engine, &scene, 12.0),
                "{kind:?} reverses exactly"
            );
        }
    }
}

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene, height: f32) -> Vec<u8> {
    engine
        .render_image(
            scene,
            &Camera {
                eye: Vec3::new(0.0, 0.0, 20.0),
                target: Vec3::ZERO,
                up: Vec3::Y,
                projection: Projection::Orthographic {
                    height,
                    aspect: 1.0,
                    near: 0.1,
                    far: 40.0,
                },
            },
            ImageConfig {
                width: 256,
                height: 256,
            },
        )
        .expect("polymer stroke render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn zero_radius_traces_and_tubes_keep_pixel_width_when_zooming_and_follow_trajectory() {
    for kind in [RepresentationKind::Trace, RepresentationKind::Tube] {
        let source = source();
        let mut scene = Scene::from_structure(&source).expect("scene");
        let owner = scene.structures().next().expect("owner").0;
        let selection = scene.add_selection(AtomSelection::All);
        scene
            .represent(
                selection,
                molgfx_core::RepresentationConfig::new(kind)
                    .tube_radius(0.0)
                    .expect("stroke")
                    .line_width(3.0),
            )
            .expect("curve");
        let start: Arc<[[f32; 3]]> = Arc::from(source.coordinates());
        let moved: Arc<[[f32; 3]]> = start
            .iter()
            .map(|point| [point[0], point[1] + 2.0, point[2]])
            .collect();
        scene
            .set_trajectory_segment(
                owner,
                TrajectorySegment::new(
                    TrajectoryFrame::new(0, 0.0, start, "start").expect("start"),
                    TrajectoryFrame::new(1, 1.0, moved, "moved").expect("end"),
                    0.0,
                )
                .expect("segment"),
            )
            .expect("trajectory");
        let mut engine = Engine::<WgpuDevice>::new(
            &EngineConfig {
                mode: RenderMode::Realtime,
                profile: RenderProfile::bare()
                    .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
                ..EngineConfig::default()
            },
            None,
        )
        .expect("GPU");
        let near = pixels(&mut engine, &scene, 12.0);
        let far = pixels(&mut engine, &scene, 24.0);
        let coverage = |image: &[u8]| {
            image
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[3] > 128)
                .count()
        };
        let near_count = coverage(&near);
        let far_count = coverage(&far);
        assert!(near_count > 200, "{kind:?} must draw at zero radius");
        assert!(
            near_count > far_count * 17 / 10 && near_count < far_count * 23 / 10,
            "{kind:?} must keep pixel width: {near_count} vs {far_count}"
        );
        scene.set_trajectory_time(owner, 1.0).expect("move");
        assert_ne!(near, pixels(&mut engine, &scene, 12.0));
        scene.set_trajectory_time(owner, 0.0).expect("reverse");
        assert_eq!(near, pixels(&mut engine, &scene, 12.0));
    }
}

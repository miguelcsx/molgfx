//! Stable visibility across batches, empty groups and a partial final workgroup.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    Relation, RelationBatch, RelationStyle, Scene, SourceNamespace, SourceRows, SpatialAnchor,
};
use molgfx_math::{Camera, Projection, Rgba8, Vec3};
use molgfx_render::{
    BackdropStyle, Engine, EngineConfig, ImageConfig, PresentationEffect, RenderMode, RenderProfile,
};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

fn stroke(y: f32, x: f32) -> Relation {
    Relation {
        start: SpatialAnchor::world(Vec3::new(x - 2.0, y, 0.0)).expect("start"),
        end: SpatialAnchor::world(Vec3::new(x + 2.0, y, 0.0)).expect("end"),
    }
}

fn scene(padded: bool) -> Scene {
    let mut scene = Scene::new();
    for (batch, color) in [Rgba8::new(255, 40, 20, 255), Rgba8::new(20, 100, 255, 255)]
        .into_iter()
        .enumerate()
    {
        let y = if batch == 0 { -0.5 } else { 0.5 };
        let rows: Arc<[Relation]> = if padded {
            (0..4097)
                .map(|row| {
                    if [0, 65, 4096].contains(&row) {
                        stroke(y, 0.0)
                    } else {
                        stroke(y, 100.0)
                    }
                })
                .collect()
        } else {
            Arc::from([stroke(y, 0.0); 3])
        };
        scene
            .add_relation_batch(
                RelationBatch::new(
                    Arc::clone(&rows),
                    SourceRows::ordered(
                        SourceNamespace(u64::try_from(batch).expect("batch")),
                        u32::try_from(rows.len()).expect("rows"),
                    ),
                    RelationStyle {
                        color,
                        width_pixels: 6.0,
                        opacity: 0.4,
                        ..RelationStyle::default()
                    },
                )
                .expect("batch"),
            )
            .expect("insertion");
    }
    scene
}

fn pixels(engine: &mut Engine<WgpuDevice>, scene: &Scene) -> Vec<u8> {
    engine
        .render_image(
            scene,
            &Camera {
                eye: Vec3::new(0.0, 0.0, 20.0),
                target: Vec3::ZERO,
                up: Vec3::Y,
                projection: Projection::Orthographic {
                    height: 8.0,
                    aspect: 1.0,
                    near: 0.1,
                    far: 40.0,
                },
            },
            ImageConfig {
                width: 128,
                height: 128,
            },
        )
        .expect("render")
        .pixels
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn sparse_visibility_keeps_exact_stroke_coverage_across_batches_and_partial_groups() {
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
    let reference = pixels(&mut engine, &scene(false));
    assert!(
        reference
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0)
    );
    let padded = scene(true);
    for _ in 0..4 {
        assert_eq!(
            reference,
            pixels(&mut engine, &padded),
            "empty groups and hidden rows must never duplicate, lose or reorder visible strokes"
        );
    }
    assert_eq!(
        reference,
        pixels(&mut engine, &scene(false)),
        "shrinking the retained scratch must preserve the active row count"
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_stroke_outside_the_centerline_frustum_keeps_its_visible_pixel_width() {
    let mut scene = Scene::new();
    scene
        .add_relation_batch(
            RelationBatch::new(
                Arc::from([Relation {
                    start: SpatialAnchor::world(Vec3::new(4.05, -2.0, 0.0)).expect("start"),
                    end: SpatialAnchor::world(Vec3::new(4.05, 2.0, 0.0)).expect("end"),
                }]),
                SourceRows::ordered(SourceNamespace(5), 1),
                RelationStyle {
                    width_pixels: 6.0,
                    ..RelationStyle::default()
                },
            )
            .expect("batch"),
        )
        .expect("insertion");
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
    let image = pixels(&mut engine, &scene);
    assert!(
        image.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0),
        "the stroke radius intersects the right edge even when both endpoints lie outside it"
    );
    for (row, pixel) in image.as_chunks::<4>().0.iter().enumerate() {
        if pixel[3] != 0 {
            assert!(row % 128 >= 124, "coverage stays at the right edge");
        }
    }
}

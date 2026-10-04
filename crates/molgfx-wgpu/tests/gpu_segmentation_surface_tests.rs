//! Actual categorical boundary rendering and exact pick identities.

#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{
    Representation, Scene, SegmentStyle, SegmentStyleTable, SegmentationPresentation,
    SegmentationStyle, SegmentedVolume, VolumeSegmentRef,
};
use molgfx_math::{Camera, Mat4, Projection, Rgba8, Vec3};
use molgfx_render::{Engine, EngineConfig, ImageConfig, PickEntity, RenderMode};
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_surface_keeps_large_integer_picks_and_style_edits_change_visible_pixels() {
    let label = (1 << 24) + 1;
    let mut labels = vec![0; 9 * 9 * 9];
    for z in 0_u8..9 {
        for y in 0_u8..9 {
            for x in 0_u8..9 {
                let point = Vec3::new(f32::from(x), f32::from(y), f32::from(z));
                if point.distance(Vec3::splat(4.0)) <= 2.5 {
                    labels[usize::from(x) + 9 * (usize::from(y) + 9 * usize::from(z))] = label;
                }
            }
        }
    }
    let mut scene = Scene::new();
    let volume = scene.add_segmented_volume(
        SegmentedVolume::new([9; 3], Mat4::IDENTITY, Arc::from(labels)).unwrap(),
    );
    let style = |color| SegmentStyleTable::new(&[SegmentStyle::new(label, color, 1.0)]).unwrap();
    let representation = scene
        .represent(
            volume,
            Representation::segmentation().segmentation_style(SegmentationStyle {
                presentation: SegmentationPresentation::Surface,
                styles: style(Rgba8::opaque(230, 40, 30)),
                ..SegmentationStyle::default()
            }),
        )
        .unwrap();
    let camera = Camera {
        eye: Vec3::new(4.0, 4.0, 20.0),
        target: Vec3::splat(4.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 8.0,
            aspect: 1.0,
            near: 0.1,
            far: 30.0,
        },
    };
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let config = ImageConfig {
        width: 96,
        height: 96,
    };
    let red = engine.render_image(&scene, &camera, config).unwrap();
    let pixel = &red.pixels[(48 * 96 + 48) * 4..][..4];
    assert!(pixel[0] > pixel[2], "surface must be red: {pixel:?}");
    let pick = engine.pick(48, 48).unwrap().unwrap();
    assert_eq!(
        pick.entity,
        PickEntity::VolumeSegment(VolumeSegmentRef { volume, label })
    );
    scene
        .representation_mut(representation)
        .unwrap()
        .segmentation
        .styles = style(Rgba8::opaque(30, 40, 230));
    let blue = engine.render_image(&scene, &camera, config).unwrap();
    let pixel = &blue.pixels[(48 * 96 + 48) * 4..][..4];
    assert!(pixel[2] > pixel[0], "surface must become blue: {pixel:?}");
    assert_eq!(
        engine.pick(48, 48).unwrap().unwrap().entity,
        PickEntity::VolumeSegment(VolumeSegmentRef { volume, label })
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn surface_picking_selects_the_nearest_label_from_either_camera_direction() {
    let mut labels = vec![0; 9 * 9 * 9];
    for z in [2, 6] {
        for y in 3..6 {
            for x in 3..6 {
                labels[x + 9 * (y + 9 * z)] = if z == 2 { 11 } else { 29 };
            }
        }
    }
    let mut scene = Scene::new();
    let volume = scene.add_segmented_volume(
        SegmentedVolume::new([9; 3], Mat4::IDENTITY, Arc::from(labels)).unwrap(),
    );
    scene
        .represent(
            volume,
            Representation::segmentation().segmentation_style(SegmentationStyle {
                presentation: SegmentationPresentation::Surface,
                styles: SegmentStyleTable::new(&[
                    SegmentStyle::new(11, Rgba8::opaque(230, 30, 40), 0.5),
                    SegmentStyle::new(29, Rgba8::opaque(30, 40, 230), 0.5),
                ])
                .unwrap(),
                ..SegmentationStyle::default()
            }),
        )
        .unwrap();
    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    for (eye_z, expected) in [(20.0, 29), (-12.0, 11)] {
        let camera = Camera {
            eye: Vec3::new(4.0, 4.0, eye_z),
            target: Vec3::splat(4.0),
            up: Vec3::Y,
            projection: Projection::Orthographic {
                height: 8.0,
                aspect: 1.0,
                near: 0.1,
                far: 30.0,
            },
        };
        let _ = engine
            .render_image(
                &scene,
                &camera,
                ImageConfig {
                    width: 96,
                    height: 96,
                },
            )
            .unwrap();
        assert_eq!(
            engine.pick(48, 48).unwrap().unwrap().entity,
            PickEntity::VolumeSegment(VolumeSegmentRef {
                volume,
                label: expected
            })
        );
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn overlapping_maps_pick_the_nearest_source_in_both_presentations() {
    for presentation in [
        SegmentationPresentation::Surface,
        SegmentationPresentation::Direct,
    ] {
        let mut scene = Scene::new();
        let mut identities = Vec::new();
        // The farther map is inserted last, so draw order cannot identify the hit.
        for z in [6, 2] {
            let mut labels = vec![0; 9 * 9 * 9];
            for y in 3..6 {
                for x in 3..6 {
                    labels[x + 9 * (y + 9 * z)] = 7;
                }
            }
            let source = scene.add_segmented_volume(
                SegmentedVolume::new([9; 3], Mat4::IDENTITY, Arc::from(labels)).unwrap(),
            );
            scene
                .represent(
                    source,
                    Representation::segmentation().segmentation_style(SegmentationStyle {
                        presentation,
                        styles: SegmentStyleTable::new(&[SegmentStyle::new(7, Rgba8::WHITE, 0.5)])
                            .unwrap(),
                        ..SegmentationStyle::default()
                    }),
                )
                .unwrap();
            identities.push(source);
        }
        let mut engine = Engine::<WgpuDevice>::new(
            &EngineConfig {
                mode: RenderMode::Realtime,
                ..EngineConfig::default()
            },
            None,
        )
        .unwrap();
        for (eye_z, expected) in [(20.0, identities[0]), (-12.0, identities[1])] {
            let camera = Camera {
                eye: Vec3::new(4.0, 4.0, eye_z),
                target: Vec3::splat(4.0),
                up: Vec3::Y,
                projection: Projection::Orthographic {
                    height: 8.0,
                    aspect: 1.0,
                    near: 0.1,
                    far: 30.0,
                },
            };
            engine
                .render_image(
                    &scene,
                    &camera,
                    ImageConfig {
                        width: 96,
                        height: 96,
                    },
                )
                .unwrap();
            assert_eq!(
                engine.pick(48, 48).unwrap().unwrap().entity,
                PickEntity::VolumeSegment(VolumeSegmentRef {
                    volume: expected,
                    label: 7
                })
            );
        }
    }
}

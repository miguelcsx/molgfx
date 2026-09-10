use super::tests::{camera, engine, structure};
use crate::engine::RenderMode;
use pdviewx_core::{LicoriceTemplate, LigandPose, Scene};
use pdviewx_math::{Quat, Rgba8, Vec3};

fn pose(index: u32, opacity: f32) -> LigandPose {
    let result = LigandPose::new(
        Vec3::new(index_position(index), 0.0, 0.0),
        Quat::IDENTITY,
        Rgba8::opaque(80, 170, 240),
        opacity,
    );
    let Ok(pose) = result else {
        panic!("valid ligand pose expected");
    };
    pose
}

#[test]
fn realtime_pose_sampling_is_bounded_deterministic_and_opacity_correct() {
    let scene = ligand_scene(10_000, true);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("compact ligand batch renders: {error}");
    }
    assert!(
        engine.scene_gpu.primitive_groups().is_none(),
        "pose topology must never lower into PrimitiveGpu rows"
    );
    let Some((_, _, groups)) = engine.scene_gpu.ligand_pose_draws() else {
        panic!("compact ligand draws exist");
    };
    assert_eq!(groups.len(), 4, "opaque/translucent spheres and capsules");
    assert_eq!(
        groups
            .iter()
            .map(|group| group.instances)
            .collect::<Vec<_>>(),
        vec![2_730, 1_365, 2_730, 1_365],
        "one batch allocation is split proportionally between opacity classes"
    );
    assert_eq!(
        engine.ligand_pose_stats(),
        super::LigandPoseStats {
            resident_poses: 10_000,
            visible_poses: 10_000,
            selected_poses: 2_730,
            beauty_instances: 8_190,
            shadow_instances: 4_095,
            draw_groups: 6,
        }
    );
    let first_groups = groups.to_vec();
    let before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };

    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("stable compact ligand batch renders: {error}");
    }
    let Some((_, _, stable_groups)) = engine.scene_gpu.ligand_pose_draws() else {
        panic!("stable compact ligand draws exist");
    };
    assert_eq!(stable_groups, first_groups);
    let Ok(writes) = engine.device.log.writes.lock() else {
        panic!("write log lock");
    };
    assert_eq!(
        writes.len() - before,
        1,
        "stable pose tables add no upload beyond frame uniforms"
    );
    drop(writes);

    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("draw log lock");
    };
    assert_eq!(
        draws.len(),
        12,
        "two realtime frames each draw four classes plus two shadow groups"
    );
    assert_eq!(
        draws.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
        vec![0, 16, 0, 16, 32, 48, 0, 16, 0, 16, 32, 48],
        "stable frames reuse the same shared argument records"
    );
}

#[test]
fn zero_alpha_poses_use_no_budget_and_fractional_alpha_routes_to_oit() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture owner exists");
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO, Vec3::X], &[[0, 1]]) {
        Ok(template) => template,
        Err(error) => panic!("ligand template validates: {error}"),
    };
    let poses = vec![
        pose(0, 1.0),
        pose(1, 0.9995),
        pose(2, 0.0),
        colored_pose(3, 0, 1.0),
    ];
    if let Err(error) = scene.add_licorice_poses(owner, template, poses) {
        panic!("ligand batch stores: {error}");
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("compact ligand batch renders: {error}");
    }
    let Some((_, _, groups)) = engine.scene_gpu.ligand_pose_draws() else {
        panic!("compact ligand draws exist");
    };
    assert_eq!(groups.len(), 4);
    assert_eq!(
        groups
            .iter()
            .map(|group| group.instances)
            .collect::<Vec<_>>(),
        vec![2, 1, 2, 1],
        "only one opaque and one fractional-alpha pose reach the GPU draws"
    );
}

#[test]
fn many_small_batches_have_a_fixed_realtime_raster_draw_ceiling() {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture owner exists");
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO, Vec3::X], &[[0, 1]]) {
        Ok(template) => template,
        Err(error) => panic!("ligand template validates: {error}"),
    };
    for index in 0..200 {
        if let Err(error) =
            scene.add_licorice_poses(owner, template.clone(), vec![pose(index, 1.0)])
        {
            panic!("ligand batch stores: {error}");
        }
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("compact ligand batches render: {error}");
    }
    let Some((_, _, groups)) = engine.scene_gpu.ligand_pose_draws() else {
        panic!("compact ligand draws exist");
    };
    assert_eq!(groups.len(), 128, "beauty draw groups are globally capped");
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("draw log lock");
    };
    assert_eq!(
        draws.len(),
        256,
        "opaque shadow repeats stay within the documented raster ceiling"
    );
}

#[test]
fn quality_mode_switch_restores_every_pose_without_rebuilding_the_scene() {
    let scene = ligand_scene(10_000, true);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("realtime compact ligand batch renders: {error}");
    }
    engine.set_render_mode(RenderMode::Cinematic);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("quality compact ligand batch renders: {error}");
    }
    let Some((_, _, groups)) = engine.scene_gpu.ligand_pose_draws() else {
        panic!("quality compact ligand draws exist");
    };
    assert_eq!(
        groups
            .iter()
            .map(|group| u64::from(group.instances))
            .sum::<u64>(),
        30_000,
        "quality keeps all two-atom and one-bond pose instances"
    );
}

#[test]
fn realtime_keeps_a_four_hundred_pose_batch_exact_when_it_fits() {
    let scene = ligand_scene(400, false);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("compact ligand batch renders: {error}");
    }
    let Some((_, _, groups)) = engine.scene_gpu.ligand_pose_draws() else {
        panic!("compact ligand draws exist");
    };
    assert_eq!(
        groups
            .iter()
            .map(|group| u64::from(group.instances))
            .sum::<u64>(),
        1_200,
    );
    assert_eq!(
        engine.ligand_pose_stats(),
        super::LigandPoseStats {
            resident_poses: 400,
            visible_poses: 400,
            selected_poses: 400,
            beauty_instances: 1_200,
            shadow_instances: 1_200,
            draw_groups: 4,
        }
    );
}

#[test]
fn small_opaque_ligand_batches_cast_grouped_sphere_and_capsule_shadows() {
    let scene = ligand_scene(20, false);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("compact ligand batch renders: {error}");
    }
    assert!(engine.scene_gpu.ligand_pose_shadow_draws(false).is_some());
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("draw log lock");
    };
    assert_eq!(
        draws.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
        vec![0, 16, 0, 16],
        "shadow and beauty each consume one sphere and one capsule range"
    );
}

fn ligand_scene(count: u32, alternating_translucency: bool) -> Scene {
    let source = structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture owner exists");
    };
    let template = match LicoriceTemplate::new(Vec3::ZERO, vec![Vec3::ZERO, Vec3::X], &[[0, 1]]) {
        Ok(template) => template,
        Err(error) => panic!("ligand template validates: {error}"),
    };
    let poses = (0..count)
        .map(|index| {
            let opacity = if alternating_translucency && index % 2 != 0 {
                0.5
            } else {
                1.0
            };
            pose(index, opacity)
        })
        .collect();
    if let Err(error) = scene.add_licorice_poses(owner, template, poses) {
        panic!("ligand batch stores: {error}");
    }

    scene
}

fn colored_pose(index: u32, alpha: u8, opacity: f32) -> LigandPose {
    let result = LigandPose::new(
        Vec3::new(index_position(index), 0.0, 0.0),
        Quat::IDENTITY,
        Rgba8::new(80, 170, 240, alpha),
        opacity,
    );
    let Ok(pose) = result else {
        panic!("valid ligand pose expected");
    };
    pose
}

fn index_position(index: u32) -> f32 {
    let Ok(index) = u16::try_from(index) else {
        panic!("test pose index fits u16");
    };
    f32::from(index) * 0.01
}

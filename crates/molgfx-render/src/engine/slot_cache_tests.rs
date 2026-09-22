use super::tests::{camera, engine, represented_scene};
use molgfx_core::RepresentationKind;

#[test]
fn hide_and_show_retain_the_resident_representation_resources() {
    let mut scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("initial frame renders: {error}")
    }
    let buffer_count = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };

    scene.hide(representation);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("hidden frame renders: {error}")
    }
    assert_eq!(engine.scene_gpu.atom_draws(false).count(), 0);
    let hidden_buffer_count = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };
    assert_eq!(hidden_buffer_count, buffer_count);

    scene.show(representation);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("restored frame renders: {error}")
    }
    assert_eq!(engine.scene_gpu.atom_draws(false).count(), 1);
    let restored_buffer_count = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };
    assert_eq!(restored_buffer_count, buffer_count);
}

#[test]
fn visibility_changes_do_not_reupload_representation_records() {
    let mut scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("initial frame renders: {error}")
    }
    let writes_before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };

    scene.hide(representation);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("hidden frame renders: {error}")
    }
    scene.show(representation);
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("restored frame renders: {error}")
    }
    let writes_after = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };
    assert_eq!(
        writes_after - writes_before,
        2,
        "only frame uniforms change"
    );
}

#[test]
fn opacity_changes_write_only_fixed_size_presentation_state() {
    let mut scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("initial frame renders: {error}")
    }
    let writes_before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };
    let buffers_before = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };

    let Some(value) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    value.material.opacity = 0.4;
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("transparent frame renders: {error}")
    }

    let changed = match engine.device.log.writes.lock() {
        Ok(writes) => writes[writes_before..].to_vec(),
        Err(error) => panic!("write log lock: {error}"),
    };
    let buffers = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.clone(),
        Err(error) => panic!("buffer log lock: {error}"),
    };
    assert_eq!(buffers.len(), buffers_before);
    assert_eq!(changed.len(), 2, "frame and presentation uniforms change");
    for (buffer, _, _, _) in changed {
        let label = buffers
            .iter()
            .find(|(id, _, _)| *id == buffer)
            .map(|(_, label, _)| *label);
        assert!(
            matches!(label, Some("frame uniforms" | "representation uniforms")),
            "opacity wrote unexpected buffer {label:?}"
        );
    }
}

#[test]
fn cartoon_opacity_changes_do_not_rebuild_ribbon_geometry() {
    let mut scene = represented_scene(1, 1);
    let Some((representation, _)) = scene.representations().next() else {
        panic!("fixture has a representation")
    };
    let Some(value) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    value.kind = RepresentationKind::Cartoon;
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("initial frame renders: {error}")
    }
    let writes_before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };
    let buffers_before = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };

    let Some(value) = scene.representation_mut(representation) else {
        panic!("representation resolves")
    };
    value.material.opacity = 0.4;
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("transparent frame renders: {error}")
    }

    let changed = match engine.device.log.writes.lock() {
        Ok(writes) => writes[writes_before..].to_vec(),
        Err(error) => panic!("write log lock: {error}"),
    };
    let buffers = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.clone(),
        Err(error) => panic!("buffer log lock: {error}"),
    };
    assert_eq!(buffers.len(), buffers_before);
    assert_eq!(changed.len(), 2, "frame and cartoon uniforms change");
    for (buffer, _, _, _) in changed {
        let label = buffers
            .iter()
            .find(|(id, _, _)| *id == buffer)
            .map(|(_, label, _)| *label);
        assert!(
            matches!(label, Some("frame uniforms" | "cartoon clipping uniforms")),
            "opacity wrote unexpected buffer {label:?}"
        );
    }
}

#[test]
fn a_stable_frame_allocates_no_buffers_and_uploads_only_frame_state() {
    let scene = represented_scene(1, 4);
    let mut engine = engine();
    // Two warm-up frames: the first builds pools and uploads everything, the
    // second settles anything the first deferred.
    for _ in 0..2 {
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("warm-up frame renders: {error}")
        }
    }
    let buffers_before = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };
    let writes_before = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };

    for _ in 0..8 {
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("stable frame renders: {error}")
        }
    }

    let buffers_after = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers.len(),
        Err(error) => panic!("buffer log lock: {error}"),
    };
    let writes_after = match engine.device.log.writes.lock() {
        Ok(writes) => writes.len(),
        Err(error) => panic!("write log lock: {error}"),
    };
    assert_eq!(
        buffers_after, buffers_before,
        "an unchanged scene must not allocate GPU buffers"
    );
    // Per-frame uniforms are the only thing a still scene legitimately
    // rewrites; anything proportional to the scene would scale with the
    // eight frames rather than staying a small fixed count per frame.
    let per_frame = (writes_after - writes_before) / 8;
    assert!(
        per_frame <= 2,
        "a stable frame uploaded {per_frame} buffers; only frame-level state should change"
    );
}

#[test]
fn overlapping_representations_do_not_multiply_stable_frame_cost() {
    // Four representations over one structure must not make a still frame four
    // times as expensive: every slot's revisions are unchanged, so each costs a
    // comparison rather than a re-upload.
    let mut costs = Vec::new();
    for count in [1_usize, 4] {
        let scene = represented_scene(1, count);
        let mut engine = engine();
        for _ in 0..2 {
            if let Err(error) = engine.render(&scene, &camera()) {
                panic!("warm-up frame renders: {error}")
            }
        }
        let before = match engine.device.log.writes.lock() {
            Ok(writes) => writes.len(),
            Err(error) => panic!("write log lock: {error}"),
        };
        for _ in 0..8 {
            if let Err(error) = engine.render(&scene, &camera()) {
                panic!("stable frame renders: {error}")
            }
        }
        let after = match engine.device.log.writes.lock() {
            Ok(writes) => writes.len(),
            Err(error) => panic!("write log lock: {error}"),
        };
        costs.push(after - before);
    }
    let (Some(one), Some(four)) = (costs.first(), costs.get(1)) else {
        panic!("both scene sizes were measured")
    };
    assert_eq!(
        one, four,
        "stable-frame uploads must not grow with representation count"
    );
}

#[test]
fn structure_wide_resources_are_shared_by_every_representation() {
    // What a representation may not duplicate: anything derived from the
    // molecule rather than from how the molecule is being drawn.
    const SHARED: [&str; 4] = [
        "immutable asset arena",
        "visual property table",
        "model transform",
        "visual parameter arena",
    ];
    for count in [1_usize, 8] {
        let scene = represented_scene(1, count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(buffers) = engine.device.log.buffers.lock() else {
            panic!("buffer log lock")
        };
        for label in SHARED {
            let created = buffers.iter().filter(|entry| entry.1 == label).count();
            assert_eq!(
                created, 1,
                "{count} representations created {created} '{label}' buffers; \
                 structure-wide resources are shared"
            );
        }
    }
}

#[test]
fn per_representation_gpu_cost_stays_within_its_budget() {
    // Each representation still gets its own packed records, compaction map,
    // visibility lists and indirect arguments even when it draws the same atoms
    // as its neighbour. Sharing those is the next ownership change; until then
    // this pins the cost so it cannot quietly grow.
    const BUDGET: usize = 13;
    let mut counts = Vec::new();
    for count in [1_usize, 8] {
        let scene = represented_scene(1, count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(buffers) = engine.device.log.buffers.lock() else {
            panic!("buffer log lock")
        };
        counts.push(buffers.len());
    }
    let (Some(one), Some(eight)) = (counts.first(), counts.get(1)) else {
        panic!("both scene sizes were measured")
    };
    let per_representation = (eight - one) / 7;
    assert!(
        per_representation <= BUDGET,
        "each representation now costs {per_representation} GPU buffers, over the \
         budget of {BUDGET}"
    );
}

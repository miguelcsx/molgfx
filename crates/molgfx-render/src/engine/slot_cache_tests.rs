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
    // Three fixed-size blocks: the frame, the representation's presentation
    // parameters, and the colour scheme block, which carries the same
    // presentation state. Nothing proportional to the scene is touched.
    assert_eq!(changed.len(), 3, "only fixed-size uniforms change");
    for (buffer, _, _, _) in changed {
        let label = buffers
            .iter()
            .find(|(id, _, _)| *id == buffer)
            .map(|(_, label, _)| *label);
        assert!(
            matches!(
                label,
                Some("frame uniforms" | "representation uniforms" | "colour scheme uniforms")
            ),
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
fn one_packed_record_set_serves_every_representation_over_one_selection() {
    // The identity change: eight representations over one query share one
    // packed record set, one compaction map and one bond buffer.
    for count in [1_usize, 8] {
        let scene = represented_scene(1, count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(buffers) = engine.device.log.buffers.lock() else {
            panic!("buffer log lock")
        };
        for label in [
            "atom instances",
            "bond instances",
            "source-to-compacted atom indices",
        ] {
            let created = buffers.iter().filter(|entry| entry.1 == label).count();
            assert_eq!(
                created, 1,
                "{count} representations created {created} '{label}' buffers; packed \
                 records are shared by canonical selection"
            );
        }
    }
}

#[test]
fn four_representations_with_one_visibility_key_dispatch_one_cull() {
    // Culling is a function of the records and the cull policy, not of how the
    // representation is drawn, so four identical representations compute one
    // visible set and issue one dispatch between them.
    let mut counts = Vec::new();
    for representation_count in [1_usize, 4] {
        let scene = represented_scene(1, representation_count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(passes) = engine.device.log.compute_passes.lock() else {
            panic!("pass log lock")
        };
        let culls = passes
            .iter()
            .filter(|label| **label == "visibility culling")
            .count();
        counts.push(culls);
    }
    let (Some(one), Some(four)) = (counts.first(), counts.get(1)) else {
        panic!("both scene sizes were measured")
    };
    assert_eq!(
        one, four,
        "the number of cull dispatches must not grow with representation count"
    );
    let scene = represented_scene(1, 4);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(buffers) = engine.device.log.buffers.lock() else {
        panic!("buffer log lock")
    };
    assert_eq!(
        buffers
            .iter()
            .filter(|(_, label, _)| *label == "visible atom indices")
            .count(),
        1,
        "one visibility key owns one visible-atom list"
    );
}

#[test]
fn a_tight_budget_releases_the_records_of_a_dropped_selection() {
    let scene = represented_scene(1, 1);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    assert!(
        engine.derived_cache_usage().gpu_bytes > 0,
        "records are charged"
    );
    // A budget far below one record set refuses every set, so the next frame
    // evicts what it held instead of growing without bound.
    engine.set_derived_cache_budget(crate::DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 1,
    });
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame still renders under a tight budget: {error}")
    }
    assert_eq!(
        engine.derived_cache_usage().gpu_bytes,
        0,
        "an over-budget resident set is released rather than retained"
    );
}

#[test]
fn a_representation_reclaims_its_records_after_being_evicted() {
    // Eviction must be recoverable, not terminal: an evicted set is rebuilt on
    // the next frame that needs it, and the frame still renders. Without this
    // the budget would be a correctness hazard rather than a memory bound.
    let scene = represented_scene(1, 1);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("first frame renders: {error}")
    }
    let charged = engine.derived_cache_usage().gpu_bytes;
    assert!(charged > 0, "records are charged once resident");

    // A budget that admits nothing evicts what the first frame built.
    engine.set_derived_cache_budget(crate::DerivedCacheBudget {
        cpu_bytes: 0,
        gpu_bytes: 1,
    });
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame under a tight budget renders: {error}")
    }
    assert_eq!(
        engine.derived_cache_usage().gpu_bytes,
        0,
        "the evicted set is released"
    );

    // The next frame must re-materialise it rather than draw from nothing.
    engine.set_derived_cache_budget(crate::DerivedCacheBudget::default());
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("the frame after re-admitting the budget renders: {error}")
    }
    assert!(
        engine.derived_cache_usage().gpu_bytes > 0,
        "an evicted record set is rebuilt by the frame that needs it"
    );
}

#[test]
fn per_representation_gpu_cost_stays_within_its_budget() {
    // What one representation may still own privately, measured by diffing the
    // buffer log at one and at eight representations:
    //
    //   representation uniforms          its uniform block
    //   visual configuration             its visual state
    //   visual entity results            its per-entity visual results
    //   colour scheme uniforms           its resolved colour scheme
    //   disabled fragment visual program  a zeroed placeholder
    //
    // Everything a representation shares — packed records, compaction, the
    // quality hierarchy, visibility lists, indirect arguments and the surface
    // field — is keyed by canonical selection and must not appear here, which
    // is what the other tests in this file assert one by one.
    //
    // Five, not four: the zeroed fragment program is still per slot and is the
    // one buffer here that could be the scene-wide fallback instead. Narrowing
    // the budget to four is that change, not a lower constant.
    const BUDGET: usize = 5;
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

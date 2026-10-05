use crate::engine::tests::{camera, engine, represented_scene};
use molgfx_core::RepresentationKind;
use std::collections::BTreeSet;

#[test]
fn one_indirect_argument_arena_serves_every_representation() {
    // Three small argument buffers per representation become one scene-wide
    // arena with a per-key slot, so the portable buffer budget is spent on
    // geometry rather than on argument records.
    let mut counts = Vec::new();
    for representation_count in [1_usize, 8] {
        let scene = represented_scene(1, representation_count);
        let mut engine = engine();
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(buffers) = engine.device.log.buffers.lock() else {
            panic!("buffer log lock")
        };
        counts.push(
            buffers
                .iter()
                .filter(|(_, label, _)| *label == "indirect draw arguments")
                .count(),
        );
    }
    let (Some(one), Some(eight)) = (counts.first(), counts.get(1)) else {
        panic!("both scene sizes were measured")
    };
    assert_eq!(one, &1, "one arena for one representation");
    assert_eq!(eight, &1, "the same arena serves eight representations");
}

#[test]
fn every_indirect_draw_reads_the_one_arena() {
    let scene = represented_scene(1, 4);
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("draw log lock")
    };
    let ids: Vec<u32> = draws.iter().map(|(id, _)| *id).collect();
    assert!(!ids.is_empty(), "the frame issues indirect draws");
    let Ok(buffers) = engine.device.log.buffers.lock() else {
        panic!("buffer log lock")
    };
    let arena = buffers
        .iter()
        .find(|(_, label, _)| *label == "indirect draw arguments")
        .map(|(id, _, _)| *id);
    assert!(
        arena.is_some_and(|arena| ids.iter().all(|id| *id == arena)),
        "every indirect draw reads the one arena"
    );
}

#[test]
fn different_cull_policies_read_different_slots_of_one_arena() {
    // A shared visibility key legitimately shares a slot; a different policy
    // must not, or one representation's cull would overwrite another's.
    let mut scene = represented_scene(1, 1);
    let selection = scene.add_selection(molgfx_core::AtomSelection::All);
    if let Err(error) = scene.represent(selection, RepresentationKind::Licorice) {
        panic!("licorice applies: {error}")
    }
    let mut engine = engine();
    if let Err(error) = engine.render(&scene, &camera()) {
        panic!("frame renders: {error}")
    }
    let Ok(draws) = engine.device.log.indirect_draws.lock() else {
        panic!("draw log lock")
    };
    let offsets: BTreeSet<u64> = draws.iter().map(|(_, offset)| *offset).collect();
    assert!(
        offsets.len() > 1,
        "atom and bond slots of one representation are distinct offsets"
    );
}

#[test]
fn many_distinct_record_sets_upload_every_argument_to_the_final_arena() {
    let mut scene = represented_scene(1, 0);
    let selection = scene.add_selection(molgfx_core::AtomSelection::All);
    for scale in 1_u16..=10 {
        let handle = scene
            .represent(selection, RepresentationKind::Spacefill)
            .expect("representation");
        scene
            .representation_mut(handle)
            .expect("representation exists")
            .params
            .radius_scale = f32::from(scale);
    }
    let mut engine = engine();
    engine.render(&scene, &camera()).expect("frame renders");
    let buffers = engine.device.log.buffers.lock().expect("buffers");
    let arenas: Vec<_> = buffers
        .iter()
        .filter(|(_, label, _)| *label == "indirect draw arguments")
        .collect();
    assert_eq!(
        arenas.len(),
        1,
        "reserve the complete frame before writing arguments"
    );
    let arena = arenas[0].0;
    let writes = engine.device.log.writes.lock().expect("writes");
    let offsets: BTreeSet<_> = writes
        .iter()
        .filter(|(id, _, size, _)| *id == arena && *size == 16)
        .map(|(_, offset, _, _)| *offset)
        .collect();
    assert_eq!(
        offsets.len(),
        30,
        "all ten sets retain their three arguments"
    );
}

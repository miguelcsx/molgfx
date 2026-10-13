use super::*;
use crate::{AtomSelection, DatasetId, StructureAsset, StructureHandle, fixture};

const HOVER: u32 = InteractionState::HOVERED.bits();

fn two_structures() -> (Scene, [StructureHandle; 2]) {
    let structure = fixture::structure();
    let asset = match StructureAsset::new(DatasetId::LEGACY, &structure) {
        Ok(asset) => asset,
        Err(error) => panic!("asset: {error}"),
    };
    let mut scene = Scene::from_asset(&asset);
    let second = scene.add_asset(&asset);
    let first = scene
        .structures()
        .map(|(handle, _)| handle)
        .find(|handle| *handle != second);
    let Some(first) = first else {
        panic!("first structure")
    };
    (scene, [first, second])
}

fn hovered(scene: &Scene, handle: StructureHandle) -> Vec<bool> {
    let Some(column) = scene.interaction_state(handle) else {
        panic!("column")
    };
    column
        .values()
        .iter()
        .map(|word| word & HOVER != 0)
        .collect()
}

#[test]
fn incremental_hover_matches_a_full_scan_for_any_sequence() {
    let (mut scene, handles) = two_structures();
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed >> 16
    };
    for _ in 0..400 {
        let pick = next() % 7;
        let (target, rows): (Option<StructureHandle>, Vec<u32>) = match pick {
            0 => (None, vec![]),
            1 | 2 => (Some(handles[(next() % 2) as usize]), vec![next() % 8]),
            3 => (Some(handles[(next() % 2) as usize]), vec![]),
            _ => (
                Some(handles[(next() % 2) as usize]),
                vec![next() % 8, next() % 8],
            ),
        };
        let before = scene.interaction_state_revision();
        let changed = match scene.update_pointer_hover(target, &rows) {
            Ok(changed) => changed,
            Err(error) => panic!("hover: {error}"),
        };
        assert_eq!(changed, scene.interaction_state_revision() != before);
        for handle in handles {
            let expected: Vec<bool> = (0..8)
                .map(|row| Some(handle) == target && rows.contains(&row))
                .collect();
            assert_eq!(hovered(&scene, handle), expected);
        }
    }
}

#[test]
fn hover_after_an_outside_channel_write_clears_the_stale_bit() {
    let (mut scene, [first, second]) = two_structures();
    assert!(matches!(
        scene.update_pointer_hover(Some(first), &[1]),
        Ok(true)
    ));
    let all = AtomSelection::All;
    assert!(
        scene
            .update_interaction_channel(second, HOVER, Some(&all))
            .is_ok()
    );
    assert!(matches!(
        scene.update_pointer_hover(Some(first), &[1]),
        Ok(true)
    ));
    assert!(hovered(&scene, second).iter().all(|bit| !bit));
    assert_eq!(hovered(&scene, first).iter().filter(|bit| **bit).count(), 1);
}

#[test]
fn hover_reports_only_the_rows_it_edited() {
    let (mut scene, [first, _]) = two_structures();
    assert!(matches!(
        scene.update_pointer_hover(Some(first), &[2]),
        Ok(true)
    ));
    let Some(seen) = scene.interaction_state(first).map(|c| c.revision().get()) else {
        panic!("column")
    };
    assert!(matches!(
        scene.update_pointer_hover(Some(first), &[5]),
        Ok(true)
    ));
    assert_eq!(scene.interaction_state_dirty(first, seen), Some(2..6));
    let Some(now) = scene.interaction_state(first).map(|c| c.revision().get()) else {
        panic!("column")
    };
    assert_eq!(scene.interaction_state_dirty(first, now), Some(0..0));
    assert_eq!(scene.interaction_state_dirty(first, now + 1), None);
}

#[test]
fn a_history_gap_demands_a_full_upload() {
    let (mut scene, [first, _]) = two_structures();
    for index in 0..20 {
        assert!(
            scene
                .update_pointer_hover(Some(first), &[index % 8])
                .is_ok()
        );
    }
    assert_eq!(scene.interaction_state_dirty(first, 0), None);
}

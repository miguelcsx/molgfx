use super::*;
use molgfx_core::{
    BrickAddress, BrickCatalog, BrickDescriptor, BrickId, BrickMetadata, BrickShape,
    BrickValueRange, ChunkId, DatasetId, DirtyGeneration,
};

fn descriptor(id: u64, origin: [u64; 3], mip: u16, generation: u64) -> BrickDescriptor {
    let shape = match BrickShape::new([10; 3], 1) {
        Ok(value) => value,
        Err(error) => panic!("fixture shape rejected: {error}"),
    };
    let metadata = match BrickMetadata::new(
        BrickId::new(id),
        BrickAddress { origin, mip },
        shape,
        BrickValueRange::Occupancy {
            has_empty: true,
            has_occupied: true,
        },
        DirtyGeneration::new(generation),
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture metadata rejected: {error}"),
    };
    BrickDescriptor {
        chunk: ChunkId::new(id + 100),
        metadata,
    }
}

#[test]
fn page_table_preserves_u64_identity_and_rejects_stale_generations() {
    let id = u64::from(u32::MAX) + 41;
    let mut working_set = match BrickWorkingSet::new(2) {
        Ok(value) => value,
        Err(error) => panic!("working set rejected: {error}"),
    };
    let current = descriptor(id, [0; 3], 0, 7);
    let slot = match working_set.commit(current) {
        Ok(value) => value,
        Err(error) => panic!("current page rejected: {error}"),
    };
    assert_eq!(
        working_set
            .resolve(current.metadata.address)
            .map(|page| page.slot),
        Some(slot)
    );
    assert!(matches!(
        working_set.commit(descriptor(id, [0; 3], 0, 6)),
        Err(BrickWorkingSetError::StaleGeneration {
            current: 7,
            received: 6,
            ..
        })
    ));
    assert_eq!(
        working_set
            .get(BrickId::new(id))
            .map(|page| page.metadata.generation.get()),
        Some(7)
    );
}

#[test]
fn atlas_and_selection_capacity_apply_typed_backpressure() {
    let mut working_set = match BrickWorkingSet::new(1) {
        Ok(value) => value,
        Err(error) => panic!("working set rejected: {error}"),
    };
    assert!(working_set.commit(descriptor(1, [0; 3], 0, 1)).is_ok());
    assert!(matches!(
        working_set.commit(descriptor(2, [16, 0, 0], 0, 1)),
        Err(BrickWorkingSetError::CapacityExceeded {
            resource: "brick atlas",
            capacity: 1
        })
    ));

    let catalog = match BrickCatalog::new(
        DatasetId::new(4),
        [128; 3],
        1,
        vec![descriptor(1, [0; 3], 0, 1), descriptor(2, [16, 0, 0], 0, 1)],
    ) {
        Ok(value) => value,
        Err(error) => panic!("catalog rejected: {error}"),
    };
    let selector = match ClipmapSelector::new(vec![ClipmapLevel { mip: 0, radius: 64 }]) {
        Ok(value) => value,
        Err(error) => panic!("clipmap rejected: {error}"),
    };
    let mut scratch = BrickSelectionScratch::with_capacity(1);
    assert!(matches!(
        selector.select_into(&catalog, &working_set, [0; 3], &mut scratch),
        Err(BrickWorkingSetError::CapacityExceeded {
            resource: "clipmap selection scratch",
            capacity: 1
        })
    ));
}

#[test]
fn clipmap_selection_reuses_scratch_and_marks_resident_pages() {
    let fine = descriptor(11, [8, 8, 8], 0, 1);
    let coarse = descriptor(12, [4, 4, 4], 1, 1);
    let remote = descriptor(13, [80, 80, 80], 0, 1);
    let catalog =
        match BrickCatalog::new(DatasetId::new(5), [256; 3], 4, vec![fine, coarse, remote]) {
            Ok(value) => value,
            Err(error) => panic!("catalog rejected: {error}"),
        };
    let selector = match ClipmapSelector::new(vec![
        ClipmapLevel { mip: 0, radius: 16 },
        ClipmapLevel { mip: 1, radius: 32 },
    ]) {
        Ok(value) => value,
        Err(error) => panic!("clipmap rejected: {error}"),
    };
    let mut working_set = match BrickWorkingSet::new(2) {
        Ok(value) => value,
        Err(error) => panic!("working set rejected: {error}"),
    };
    assert!(working_set.commit(fine).is_ok());
    let mut scratch = BrickSelectionScratch::with_capacity(3);
    assert!(
        selector
            .select_into(&catalog, &working_set, [12; 3], &mut scratch)
            .is_ok()
    );
    let pointer = scratch.entries().as_ptr();
    assert_eq!(scratch.entries().len(), 2);
    assert!(scratch.entries()[0].resident_slot.is_some());
    assert!(
        selector
            .select_into(&catalog, &working_set, [12; 3], &mut scratch)
            .is_ok()
    );
    assert_eq!(scratch.entries().as_ptr(), pointer);
}

#[test]
fn huge_catalog_local_query_visits_work_bounded_well_below_catalog_size() {
    const SIDE: u64 = 64;
    let descriptor_count = match usize::try_from(SIDE * SIDE * SIDE) {
        Ok(value) => value,
        Err(error) => panic!("fixture count is not addressable: {error}"),
    };
    let mut descriptors = Vec::with_capacity(descriptor_count);
    for z in 0..SIDE {
        for y in 0..SIDE {
            for x in 0..SIDE {
                let id = z * SIDE * SIDE + y * SIDE + x;
                descriptors.push(descriptor(id, [x * 8, y * 8, z * 8], 0, 1));
            }
        }
    }
    let catalog = match BrickCatalog::new(DatasetId::new(91), [SIDE * 8; 3], 1, descriptors) {
        Ok(value) => value,
        Err(error) => panic!("large catalog rejected: {error}"),
    };
    let selector = match ClipmapSelector::new(vec![ClipmapLevel { mip: 0, radius: 0 }]) {
        Ok(value) => value,
        Err(error) => panic!("clipmap rejected: {error}"),
    };
    let working_set = match BrickWorkingSet::new(0) {
        Ok(value) => value,
        Err(error) => panic!("working set rejected: {error}"),
    };
    let mut scratch = BrickSelectionScratch::with_capacity(8);
    let stats = match selector.select_into(
        &catalog,
        &working_set,
        [SIDE * 4 + 4, SIDE * 4 + 4, SIDE * 4 + 4],
        &mut scratch,
    ) {
        Ok(value) => value,
        Err(error) => panic!("local query failed: {error}"),
    };

    assert_eq!(catalog.descriptors().len(), descriptor_count);
    assert_eq!(scratch.entries().len(), 1);
    assert!(u64::from(stats.visited_descriptors) < SIDE * SIDE * SIDE / 1_000);
    assert!(stats.visited_nodes < 256);
}

#[test]
fn hierarchical_query_matches_linear_reference_across_mips_and_radii() {
    let mut descriptors = Vec::new();
    let mut id = 0;
    for mip in 0..=1 {
        for z in 0..12 {
            for y in 0..12 {
                for x in 0..12 {
                    descriptors.push(descriptor(id, [x * 8, y * 8, z * 8], mip, 1));
                    id += 1;
                }
            }
        }
    }
    let catalog = match BrickCatalog::new(DatasetId::new(92), [256; 3], 1, descriptors) {
        Ok(value) => value,
        Err(error) => panic!("differential catalog rejected: {error}"),
    };
    let levels = [
        ClipmapLevel { mip: 0, radius: 0 },
        ClipmapLevel { mip: 1, radius: 9 },
    ];
    let selector = match ClipmapSelector::new(levels.to_vec()) {
        Ok(value) => value,
        Err(error) => panic!("clipmap rejected: {error}"),
    };
    let working_set = match BrickWorkingSet::new(0) {
        Ok(value) => value,
        Err(error) => panic!("working set rejected: {error}"),
    };
    let mut scratch = BrickSelectionScratch::with_capacity(catalog.descriptors().len());

    for focus in [[0, 0, 0], [47, 53, 71], [95, 95, 95], [191, 127, 63]] {
        let stats = match selector.select_into(&catalog, &working_set, focus, &mut scratch) {
            Ok(value) => value,
            Err(error) => panic!("indexed query failed: {error}"),
        };
        let indexed = scratch
            .entries()
            .iter()
            .map(|entry| (entry.descriptor.metadata.id, entry.distance))
            .collect::<Vec<_>>();
        let linear = linear_reference(&catalog, &levels, focus);
        assert_eq!(indexed, linear);
        assert_eq!(
            usize::try_from(stats.matched_descriptors).ok(),
            Some(linear.len())
        );
    }
}

fn linear_reference(
    catalog: &BrickCatalog,
    levels: &[ClipmapLevel],
    focus: [u64; 3],
) -> Vec<(BrickId, u64)> {
    let mut selected = Vec::new();
    for descriptor in catalog.descriptors() {
        let Some(level) = levels
            .iter()
            .find(|level| level.mip == descriptor.metadata.address.mip)
        else {
            continue;
        };
        let distance = reference_distance(*descriptor, focus);
        if distance <= level.radius {
            selected.push((
                descriptor.metadata.address.mip,
                descriptor.metadata.id,
                distance,
            ));
        }
    }
    selected.sort_unstable_by_key(|(mip, id, distance)| (*mip, *distance, *id));
    selected
        .into_iter()
        .map(|(_, id, distance)| (id, distance))
        .collect()
}

fn reference_distance(descriptor: BrickDescriptor, focus: [u64; 3]) -> u64 {
    let mip_focus = focus.map(|value| value >> descriptor.metadata.address.mip);
    descriptor
        .metadata
        .address
        .origin
        .into_iter()
        .zip(descriptor.metadata.shape.interior())
        .zip(mip_focus)
        .map(|((start, width), point)| {
            let end = start + u64::from(width);
            if point < start {
                start - point
            } else if point >= end {
                point - end + 1
            } else {
                0
            }
        })
        .fold(0, u64::max)
}

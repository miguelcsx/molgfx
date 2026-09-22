use super::super::placement_acceleration::PlacementAcceleration;
use super::super::quality_hardware::QualityBlas;
use super::*;
use crate::testing::{MockBindGroupLayout, MockDevice};
use molgfx_core::{EntityId, PlacedStructure};
use std::sync::atomic::Ordering;

fn placed_structure() -> PlacedStructure {
    let cif = "\
data_quality
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
ATOM 1 C C1 . LIG A 1 1 0.0 0.0 0.0 1.0 10.0 1 A 1
ATOM 2 C C2 . LIG A 1 1 2.0 0.0 0.0 1.0 10.0 1 A 1
ATOM 3 O O1 . LIG A 1 1 4.0 1.0 0.0 1.0 10.0 1 A 1
";
    let structure = match molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("quality.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("quality fixture parses: {diagnostics:?}"),
    };
    match PlacedStructure::new(&structure) {
        Some(placed) => placed,
        None => panic!("quality fixture has a dense first model"),
    }
}

fn atom(source: u64) -> AtomGpu {
    let entity_id = match EntityId::pack(EntityKind::Atom, source) {
        Ok(entity_id) => entity_id,
        Err(error) => panic!("fixture atom id fits: {error}"),
    };
    AtomGpu {
        entity_id,
        ..AtomGpu::default()
    }
}

#[test]
fn bond_hierarchy_is_persistent_and_covers_each_compact_bond() {
    let device = MockDevice::default();
    let queue = device.queue();
    let placed = placed_structure();
    let atoms = [atom(0), atom(1), atom(2)];
    let bonds = [
        BondGpu {
            atom_a: 0,
            atom_b: 1,
            radius: 0.2,
            ..BondGpu::default()
        },
        BondGpu {
            atom_a: 1,
            atom_b: 2,
            radius: 0.25,
            ..BondGpu::default()
        },
    ];
    let mut acceleration = QualityAcceleration::<MockDevice>::new();
    acceleration
        .sync_topology(&device, &queue, &atoms, &bonds, &placed)
        .unwrap_or_else(|error| panic!("quality hierarchy uploads: {error}"));
    assert_eq!(acceleration.hierarchy.primitive_indices.len(), bonds.len());
    assert!(acceleration.counts().nodes > 0);
    assert_eq!(
        acceleration.upload_words.len(),
        acceleration.hierarchy.nodes.len() * 8
            + acceleration.upload_indices.len()
            + bonds.len() * 4
    );
    assert_eq!(
        &acceleration.upload_words[acceleration.upload_words.len() - bonds.len() * 4..],
        bytemuck::cast_slice::<BondGpu, u32>(&bonds)
    );
    let capacities = (
        acceleration.primitives.capacity(),
        acceleration.bounds.capacity(),
        acceleration.hierarchy.nodes.capacity(),
        acceleration.upload_indices.capacity(),
    );
    acceleration
        .sync_topology(&device, &queue, &atoms, &bonds, &placed)
        .unwrap_or_else(|error| panic!("quality hierarchy rebuilds in place: {error}"));
    assert_eq!(
        capacities,
        (
            acceleration.primitives.capacity(),
            acceleration.bounds.capacity(),
            acceleration.hierarchy.nodes.capacity(),
            acceleration.upload_indices.capacity(),
        )
    );
}

#[test]
fn empty_bond_sets_reuse_the_structure_binding_without_allocating() {
    let device = MockDevice::default();
    let queue = device.queue();
    let placed = placed_structure();
    let mut acceleration = QualityAcceleration::<MockDevice>::new();
    let atoms = [atom(0), atom(1)];
    let bonds = [BondGpu {
        atom_a: 0,
        atom_b: 1,
        radius: 0.2,
        ..BondGpu::default()
    }];
    acceleration
        .sync_topology(&device, &queue, &atoms, &bonds, &placed)
        .unwrap_or_else(|error| panic!("non-empty quality hierarchy binds: {error}"));
    assert!(acceleration.nodes().is_some());
    acceleration
        .sync_topology(&device, &queue, &atoms, &[], &placed)
        .unwrap_or_else(|error| panic!("empty quality hierarchy binds: {error}"));
    assert_eq!(acceleration.counts().nodes, 0);
    assert_eq!(acceleration.counts().indices, 0);
    assert!(acceleration.nodes().is_none());
    assert_eq!(acceleration.nodes_capacity, 0);
}

#[test]
fn one_shared_blas_serves_every_placement_and_only_the_tlas_rebuilds() {
    let device = MockDevice::with_ray_query();
    let queue = device.queue();
    let mut placed = placed_structure();
    let atoms = [atom(0), atom(1), atom(2)];
    let bonds = [BondGpu {
        atom_a: 0,
        atom_b: 1,
        radius: 0.2,
        ..BondGpu::default()
    }];
    let layout = MockBindGroupLayout::default();

    // One BLAS, shared by two placements of the same records.
    let mut shared = QualityBlas::<MockDevice>::new();
    shared.sync(&device, &queue, &atoms, &bonds, &placed);
    assert!(shared.blas().is_some(), "the shared BLAS builds");
    let mut first = PlacementAcceleration::<MockDevice>::new();
    let mut second = PlacementAcceleration::<MockDevice>::new();
    first.sync(&device, shared.blas(), Some(&layout), placed.model_to_world);
    second.sync(&device, shared.blas(), Some(&layout), placed.model_to_world);
    assert!(first.group().is_some());
    assert!(second.group().is_some());

    let mut encoder = device.create_command_encoder();
    shared.record(&mut encoder);
    first.record(&mut encoder);
    second.record(&mut encoder);
    assert_eq!(device.log.blas_builds.load(Ordering::Relaxed), 1);
    assert_eq!(device.log.tlas_builds.load(Ordering::Relaxed), 2);

    // A repeated frame rebuilds neither.
    shared.record(&mut encoder);
    first.record(&mut encoder);
    second.record(&mut encoder);
    assert_eq!(device.log.blas_builds.load(Ordering::Relaxed), 1);
    assert_eq!(device.log.tlas_builds.load(Ordering::Relaxed), 2);

    // Moving one placement re-instances its TLAS and no BLAS.
    placed.model_to_world = molgfx_math::Mat4::from_translation(molgfx_math::Vec3::X);
    first.sync(&device, shared.blas(), Some(&layout), placed.model_to_world);
    first.record(&mut encoder);
    assert_eq!(device.log.blas_builds.load(Ordering::Relaxed), 1);
    assert_eq!(device.log.tlas_builds.load(Ordering::Relaxed), 3);
}

#[test]
fn hardware_device_loss_falls_back_without_discarding_the_compute_bvh() {
    let device = MockDevice::with_ray_query();
    let queue = device.queue();
    let placed = placed_structure();
    let atoms = [atom(0), atom(1)];
    let bonds = [BondGpu {
        atom_a: 0,
        atom_b: 1,
        radius: 0.2,
        ..BondGpu::default()
    }];
    let mut acceleration = QualityAcceleration::<MockDevice>::new();
    acceleration
        .sync_topology(&device, &queue, &atoms, &bonds, &placed)
        .unwrap_or_else(|error| panic!("quality hierarchy uploads: {error}"));

    let mut shared = QualityBlas::<MockDevice>::new();
    shared.sync(&device, &queue, &atoms, &bonds, &placed);
    assert!(shared.blas().is_some(), "the shared BLAS builds");
    let mut placement = PlacementAcceleration::<MockDevice>::new();
    placement.sync(
        &device,
        shared.blas(),
        Some(&MockBindGroupLayout::default()),
        placed.model_to_world,
    );
    assert!(placement.group().is_some());

    device.fail_next_ray_query();
    placement.record(&mut device.create_command_encoder());
    assert_eq!(
        placement.failure(),
        Some(super::super::quality_hardware::HardwareFailure::DeviceLost)
    );
    assert!(placement.group().is_none());
    assert!(acceleration.nodes().is_some());
    assert!(shared.blas().is_some());
}

#[test]
fn missing_ray_query_capability_keeps_the_compute_path_selected() {
    let device = MockDevice::default();
    let queue = device.queue();
    let placed = placed_structure();
    let atoms = [atom(0), atom(1)];
    let bonds = [BondGpu {
        atom_a: 0,
        atom_b: 1,
        radius: 0.2,
        ..BondGpu::default()
    }];
    let mut acceleration = QualityAcceleration::<MockDevice>::new();
    acceleration
        .sync_topology(&device, &queue, &atoms, &bonds, &placed)
        .unwrap_or_else(|error| panic!("quality hierarchy uploads: {error}"));
    let mut shared = QualityBlas::<MockDevice>::new();
    shared.sync(&device, &queue, &atoms, &bonds, &placed);
    let mut placement = PlacementAcceleration::<MockDevice>::new();
    placement.sync(&device, None, None, placed.model_to_world);
    assert!(placement.group().is_none());
    assert_eq!(
        placement.failure(),
        Some(super::super::quality_hardware::HardwareFailure::Unavailable)
    );
    assert!(acceleration.nodes().is_some());
}

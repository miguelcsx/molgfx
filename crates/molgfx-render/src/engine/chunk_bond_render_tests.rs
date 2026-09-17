use super::chunk_residency_tests::{complete, engine, request_and_deliver, upload, Fixture};
use super::{
    BondChunkPlacement, ChunkPlacementId, ChunkPlacementStatus, ChunkRepresentation, Engine,
    StructureChunkPlacement,
};
use crate::testing::MockDevice;
use molgfx_core::{
    ChunkData, ChunkFootprint, ChunkId, DatasetId, ResidencyClass, ResidencyDetail, ResidencyKey,
    ResidencyOutput, ResidencyRequest,
};
use molgfx_math::{Mat4, Rgba8};
use std::sync::Arc;

const ATOM_DATASET: u64 = 701;
const BOND_DATASET: u64 = 709;
const ATOM_START: u64 = u32::MAX as u64 + 5_000;

#[test]
fn cross_chunk_global_endpoints_render_and_pick_in_a_separate_namespace() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let fixtures = fixtures();
    let (first, _, _) = request_and_deliver(&mut engine, fixtures.0, &mut output);
    let (second, _, _) = request_and_deliver(&mut engine, fixtures.1, &mut output);
    upload(&mut engine, first, &mut output);
    upload(&mut engine, second, &mut output);
    complete(&mut engine, &mut output);

    let (bond, _, _) = request_and_deliver(&mut engine, fixtures.2, &mut output);
    upload(&mut engine, bond, &mut output);
    complete(&mut engine, &mut output);
    let placement = licorice(bond, 11);
    if let Err(error) = engine.set_bond_chunk_placements(&[placement]) {
        panic!("bond placement must fit: {error}");
    }
    let atoms = StructureChunkPlacement {
        id: ChunkPlacementId::new(12),
        ticket: first,
        model_to_world: Mat4::IDENTITY,
        representation: ChunkRepresentation::points(2.0, Rgba8::opaque(120, 180, 220))
            .unwrap_or_else(|error| panic!("atom points: {error}")),
    };
    engine
        .set_structure_chunk_placements(&[atoms])
        .unwrap_or_else(|error| panic!("atom placement: {error}"));
    let scene = molgfx_core::Scene::new();
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("paged bonds must render: {error}");
    }
    assert_eq!(
        engine.bond_chunk_placement_status(placement.id),
        ChunkPlacementStatus::Resident
    );
    let atom_page = engine.scene_gpu.test_chunk_pick_page(
        DatasetId::new(ATOM_DATASET),
        ChunkId::new(1_001),
        molgfx_core::EntityKind::Atom,
    );
    let bond_page = engine.scene_gpu.test_chunk_pick_page(
        DatasetId::new(BOND_DATASET),
        ChunkId::new(2_001),
        molgfx_core::EntityKind::Bond,
    );
    assert!(atom_page.is_some());
    assert!(bond_page.is_some());
    assert_ne!(atom_page, bond_page);
    let args = buffer_id(&engine, "paged bond indirect command arena");
    assert_eq!(indirect_count(&engine, args), 1);
    let buffers = buffer_count(&engine);
    let writes = bond_write_count(&engine);
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("stable paged bond frame must render: {error}");
    }
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(bond_write_count(&engine), writes);
    assert_eq!(indirect_count(&engine, args), 2);
    assert_bond_pick(&mut engine, bond_page);
}

#[test]
fn missing_cross_chunk_endpoint_remains_blocked_and_nonresident() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let fixtures = fixtures();
    let (first, _, _) = request_and_deliver(&mut engine, fixtures.0, &mut output);
    upload(&mut engine, first, &mut output);
    complete(&mut engine, &mut output);
    let (bond, _, _) = request_and_deliver(&mut engine, fixtures.2, &mut output);
    if let Err(error) = engine.upload_chunk_into(bond, &mut output) {
        panic!("missing endpoints must block rather than reject: {error}");
    }
    let placement = licorice(bond, 22);
    engine
        .set_bond_chunk_placements(&[placement])
        .unwrap_or_else(|error| panic!("blocked placement: {error}"));
    assert_eq!(
        engine.bond_chunk_placement_status(placement.id),
        ChunkPlacementStatus::NotResident
    );
    assert_eq!(
        engine
            .host_working_set()
            .residency()
            .snapshot(bond.key)
            .phase,
        molgfx_core::ResidencyPhase::Uploading
    );
}

#[test]
fn bond_delivered_before_atoms_retries_without_redelivery_or_restage() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let fixtures = fixtures();
    let (bond, _, _) = request_and_deliver(&mut engine, fixtures.2, &mut output);
    upload(&mut engine, bond, &mut output);
    let (first, _, _) = request_and_deliver(&mut engine, fixtures.0, &mut output);
    let (second, _, _) = request_and_deliver(&mut engine, fixtures.1, &mut output);
    upload(&mut engine, first, &mut output);
    upload(&mut engine, second, &mut output);
    complete(&mut engine, &mut output);
    complete(&mut engine, &mut output);
    let placement = licorice(bond, 33);
    engine
        .set_bond_chunk_placements(&[placement])
        .unwrap_or_else(|error| panic!("retried placement: {error}"));
    assert_eq!(
        engine.bond_chunk_placement_status(placement.id),
        ChunkPlacementStatus::Resident
    );
    assert_eq!(
        engine
            .host_working_set()
            .residency()
            .snapshot(bond.key)
            .phase,
        molgfx_core::ResidencyPhase::Resident
    );
}

#[test]
fn bond_placement_capacity_is_bounded_by_the_working_set() {
    let mut engine = engine();
    let ticket = request_ticket(&fixtures().2);
    let placements: Vec<_> = (0..17).map(|id| licorice(ticket, id)).collect();
    let result = engine.set_bond_chunk_placements(&placements);
    assert!(matches!(
        result,
        Err(super::ChunkResidencyError::Placement(
            super::ChunkPlacementError::Capacity { capacity: 16 }
        ))
    ));
}

fn fixtures() -> (Fixture, Fixture, Fixture) {
    let structure = bonded_structure();
    let atom_provider = pdbiox::DatasetDescriptor::source_defined(
        pdbiox::DatasetId::new(ATOM_DATASET),
        pdbiox::PayloadKind::Structure,
        ATOM_START + 4,
        2,
        pdbiox::ChunkId::new(1_001),
        2,
    )
    .unwrap_or_else(|error| panic!("atom provider descriptor: {error}"));
    let atom_bridge = molgfx_core::ProviderDatasetBridge::new(atom_provider);
    let first = structure_fixture(&atom_bridge, structure.clone(), 0, 1_001, ATOM_START);
    let second = structure_fixture(&atom_bridge, structure.clone(), 1, 1_002, ATOM_START + 2);

    let bond_descriptor = pdbiox::ChunkDescriptor::new(
        pdbiox::DatasetId::new(BOND_DATASET),
        pdbiox::ChunkId::new(2_001),
        pdbiox::LogicalRow::new(u64::from(u32::MAX) + 9_000),
        2,
    )
    .unwrap_or_else(|error| panic!("bond descriptor: {error}"));
    let bond = pdbiox::BondChunk::shared(
        bond_descriptor,
        structure,
        0..2,
        pdbiox::DatasetId::new(ATOM_DATASET),
        pdbiox::LogicalRow::new(ATOM_START),
    )
    .unwrap_or_else(|error| panic!("bond chunk: {error}"));
    let bond_provider = pdbiox::DatasetDescriptor::source_defined(
        pdbiox::DatasetId::new(BOND_DATASET),
        pdbiox::PayloadKind::BondTopology,
        u64::from(u32::MAX) + 9_002,
        1,
        pdbiox::ChunkId::new(2_001),
        2,
    )
    .unwrap_or_else(|error| panic!("bond provider descriptor: {error}"));
    let bridge = molgfx_core::ProviderDatasetBridge::new(bond_provider);
    let footprint = ChunkFootprint::new(32, 32, 256, 256);
    let data = bridge
        .bond_chunk(
            bond,
            molgfx_core::ChunkBounds::new([-1.0; 3], [4.0; 3])
                .unwrap_or_else(|error| panic!("bond bounds: {error}")),
            footprint,
        )
        .unwrap_or_else(|error| panic!("bond bridge: {error}"));
    (
        first,
        second,
        fixture_from(data, footprint, ResidencyDetail::Atom),
    )
}

fn structure_fixture(
    bridge: &molgfx_core::ProviderDatasetBridge,
    structure: pdbiox::Structure,
    storage: usize,
    chunk: u64,
    start: u64,
) -> Fixture {
    let descriptor = pdbiox::ChunkDescriptor::new(
        pdbiox::DatasetId::new(ATOM_DATASET),
        pdbiox::ChunkId::new(chunk),
        pdbiox::LogicalRow::new(start),
        2,
    )
    .unwrap_or_else(|error| panic!("structure descriptor: {error}"));
    let source = pdbiox::StructureChunk::shared(descriptor, structure, storage)
        .unwrap_or_else(|error| panic!("structure chunk: {error}"));
    let pointer = source.positions().as_ptr() as usize;
    let bytes = bytemuck::cast_slice(source.positions()).to_vec();
    let footprint = ChunkFootprint::new(bytes.len() as u64, bytes.len() as u64 + 8, 256, 256);
    let data = bridge
        .structure_chunk(source, footprint)
        .unwrap_or_else(|error| panic!("structure bridge: {error}"));
    Fixture {
        data,
        pointer,
        bytes,
        request: request(
            data_ids(ATOM_DATASET, chunk),
            footprint,
            ResidencyDetail::Atom,
        ),
    }
}

fn fixture_from(data: ChunkData, footprint: ChunkFootprint, detail: ResidencyDetail) -> Fixture {
    let ids = data_ids(data.dataset_id().get(), data.chunk_id().get());
    Fixture {
        data,
        pointer: 0,
        bytes: Vec::new(),
        request: request(ids, footprint, detail),
    }
}

fn request(
    ids: (DatasetId, ChunkId),
    footprint: ChunkFootprint,
    detail: ResidencyDetail,
) -> ResidencyRequest {
    ResidencyRequest {
        key: ResidencyKey {
            dataset: ids.0,
            chunk: ids.1,
            detail,
        },
        footprint,
        class: ResidencyClass::Hot,
        priority: 1,
    }
}

const fn data_ids(dataset: u64, chunk: u64) -> (DatasetId, ChunkId) {
    (DatasetId::new(dataset), ChunkId::new(chunk))
}

fn request_ticket(fixture: &Fixture) -> molgfx_core::ResidencyTicket {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    request_and_deliver(&mut engine, fixture.clone(), &mut output).0
}

fn licorice(ticket: molgfx_core::ResidencyTicket, id: u64) -> BondChunkPlacement {
    BondChunkPlacement::licorice(
        ChunkPlacementId::new(id),
        ticket,
        Mat4::IDENTITY,
        0.2,
        Rgba8::opaque(210, 180, 80),
    )
    .unwrap_or_else(|error| panic!("licorice placement: {error}"))
}

fn bonded_structure() -> pdbiox::Structure {
    let mut atoms = pdbiox::ChunkBuilder::with_target(2);
    for (atom, position) in (0..4u32).zip([0.0_f32, 1.0, 2.0, 3.0]) {
        atoms.push(pdbiox::AtomRecord {
            position: Some([position, 0.0, 0.0]),
            element: pdbiox::Element::CARBON,
            atom_name: pdbiox::SymbolId::from_raw(0),
            auth_atom_name: absent(),
            alternate_component_id: absent(),
            alt_id: pdbiox::AltId::BLANK,
            residue: pdbiox::ResidueIndex::new(atom),
            occupancy: (1.0, pdbiox::Presence::Present),
            b_factor: (10.0, pdbiox::Presence::Present),
            formal_charge: (0, pdbiox::Presence::Inapplicable),
            atom_site_id: atom,
        });
    }
    let (chunks, coordinates) = atoms.finish();
    let mut bonds = pdbiox::BondTableBuilder::new();
    for (a, b) in [(1, 2), (0, 1)] {
        bonds.push(pdbiox::BondRecord {
            atom_a: pdbiox::AtomIndex::new(a),
            atom_b: pdbiox::AtomIndex::new(b),
            order: pdbiox::BondOrder::Single,
            provenance: pdbiox::BondProvenance::File,
        });
    }
    let mut data = pdbiox::StructureData::empty();
    data.chunks = Arc::new(chunks);
    data.coords = pdbiox::CoordinateStore::Single(coordinates);
    data.bonds = bonds.finish();
    pdbiox::Structure::new(data)
}

fn buffer_id(engine: &Engine<MockDevice>, label: &'static str) -> u32 {
    engine
        .device
        .log
        .buffers
        .lock()
        .map_or(u32::MAX, |buffers| {
            buffers
                .iter()
                .find(|(_, value, _)| *value == label)
                .map_or(u32::MAX, |(id, _, _)| *id)
        })
}

fn buffer_count(engine: &Engine<MockDevice>) -> usize {
    engine
        .device
        .log
        .buffers
        .lock()
        .map_or(0, |value| value.len())
}

fn bond_write_count(engine: &Engine<MockDevice>) -> usize {
    let ids = [
        buffer_id(engine, "resident provider bonds"),
        buffer_id(engine, "paged bond placements"),
        buffer_id(engine, "paged bond indirect command arena"),
    ];
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(buffer, _, _, _)| ids.contains(buffer))
            .count()
    })
}

fn indirect_count(engine: &Engine<MockDevice>, buffer: u32) -> usize {
    engine.device.log.indirect_draws.lock().map_or(0, |draws| {
        draws.iter().filter(|(id, _)| *id == buffer).count()
    })
}

fn assert_bond_pick(engine: &mut Engine<MockDevice>, page: Option<u32>) {
    let Some(page) = page else {
        panic!("bond page expected")
    };
    if let Ok(mut value) = engine.device.log.pick_resident_page.lock() {
        *value = page;
    }
    if let Ok(mut value) = engine.device.log.pick_local_row.lock() {
        *value = 0;
    }
    let picked = engine
        .pick(0, 0)
        .unwrap_or_else(|error| panic!("bond pick: {error}"));
    let Some(value) = picked else {
        panic!("bond pick expected")
    };
    let super::PickEntity::Structure(identity) = value.entity else {
        panic!("global bond expected")
    };
    assert_eq!(identity.kind(), molgfx_core::EntityKind::Bond);
    assert!(identity.row().get() > u64::from(u32::MAX));
}

fn absent<T: Default>() -> T {
    T::default()
}

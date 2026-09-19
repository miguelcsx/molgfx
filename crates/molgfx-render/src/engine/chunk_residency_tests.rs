use super::{Engine, EngineConfig};
use crate::testing::MockDevice;
use molgfx_core::{
    ChunkData, ChunkFootprint, ChunkId, ChunkPayload, DatasetId, HostWorkingSet, ResidencyBudget,
    ResidencyClass, ResidencyDetail, ResidencyKey, ResidencyOutput, ResidencyPhase,
    ResidencyRequest, ResidencyTicket,
};

const CIF: &str = "\
data_chunk
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
ATOM 1 C CA . GLY A 1 1 1.0 2.0 3.0 1.0 10.0 1 A 1
ATOM 2 N N  . GLY A 1 1 4.0 5.0 6.0 1.0 10.0 1 A 1
";

#[derive(Clone)]
pub(super) struct Fixture {
    pub(super) data: ChunkData,
    pub(super) pointer: usize,
    pub(super) bytes: Vec<u8>,
    pub(super) request: ResidencyRequest,
}

pub(super) fn engine() -> Engine<MockDevice> {
    engine_with_derived_budget(super::DerivedCacheBudget::default())
}

pub(super) fn engine_with_derived_budget(
    derived_cache: super::DerivedCacheBudget,
) -> Engine<MockDevice> {
    let mut config = EngineConfig {
        derived_cache,
        ..EngineConfig::default()
    };
    config.residency.page_size = 256;
    config.residency.page_count = 32;
    config.residency.uploads.capacity_bytes = 4096;
    config.residency.uploads.epoch_budget_bytes = 4096;
    config.residency.uploads.in_flight_budget_bytes = 4096;
    config.residency.uploads.ticket_capacity = 16;
    config.residency.uploads.alignment = 16;
    config.residency.machine_capacity = 16;
    match Engine::new(&config, None) {
        Ok(value) => value,
        Err(error) => panic!("mock engine must initialize: {error}"),
    }
}

fn backpressured_engine() -> Engine<MockDevice> {
    let mut config = EngineConfig::default();
    config.residency.page_size = 256;
    config.residency.page_count = 8;
    config.residency.uploads.capacity_bytes = 2048;
    config.residency.uploads.epoch_budget_bytes = 2048;
    config.residency.uploads.in_flight_budget_bytes = 2048;
    config.residency.uploads.ticket_capacity = 1;
    config.residency.uploads.alignment = 16;
    config.residency.machine_capacity = 4;
    match Engine::new(&config, None) {
        Ok(value) => value,
        Err(error) => panic!("backpressured engine must initialize: {error}"),
    }
}

#[test]
fn paged_gpu_storage_and_staging_are_committed_only_when_used() {
    let mut engine = engine();
    assert_eq!(
        engine
            .chunk_residency_metrics()
            .uploads
            .host_allocation_events,
        0
    );
    assert_paged_buffers_absent(&engine);

    let scene = molgfx_core::Scene::new();
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("empty frame must render without paged storage: {error}");
    }
    assert_paged_buffers_absent(&engine);

    let mut output = ResidencyOutput::default();
    let (ticket, _, _) = request_and_deliver(&mut engine, fixture(901, 903, 1), &mut output);
    upload(&mut engine, ticket, &mut output);
    assert_eq!(
        engine
            .chunk_residency_metrics()
            .uploads
            .host_allocation_events,
        2
    );
    assert!(buffer_exists(&engine, "resident structure chunks"));
    assert!(buffer_exists(&engine, "resident display coordinates"));
    assert!(buffer_exists(&engine, "resident structure chunk clusters"));
    assert!(!buffer_exists(&engine, "resident trajectory frames"));
    assert!(!buffer_exists(&engine, "resident provider bonds"));
    assert!(!buffer_exists(&engine, "paged visible rows"));
    assert!(!buffer_exists(&engine, "paged visible bonds"));
}

fn assert_paged_buffers_absent(engine: &Engine<MockDevice>) {
    for label in [
        "resident structure chunks",
        "resident display coordinates",
        "resident trajectory frames",
        "resident structure chunk clusters",
        "resident provider bonds",
        "paged visible rows",
        "paged visible bonds",
    ] {
        assert!(!buffer_exists(engine, label), "{label} must remain lazy");
    }
}

fn buffer_exists(engine: &Engine<MockDevice>, label: &'static str) -> bool {
    engine
        .device
        .log
        .buffers
        .lock()
        .is_ok_and(|buffers| buffers.iter().any(|(_, value, _)| *value == label))
}

pub(super) fn fixture(dataset: u64, chunk: u64, priority: i32) -> Fixture {
    let structure = match molframe::read_bytes(
        CIF.as_bytes().to_vec(),
        Some("chunk.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((value, _)) => value,
        Err(error) => panic!("fixture must parse: {error:?}"),
    };
    let provider = match molframe::StructureChunkProvider::new(
        molframe::DatasetId::new(dataset),
        molframe::ChunkId::new(chunk),
        structure,
    ) {
        Ok(value) => value,
        Err(error) => panic!("provider must initialize: {error}"),
    };
    let source = match provider.chunk(molframe::ChunkId::new(chunk)) {
        Ok(value) => value,
        Err(error) => panic!("provider chunk must exist: {error}"),
    };
    let pointer = source.positions().as_ptr() as usize;
    let bytes = bytemuck::cast_slice(source.positions()).to_vec();
    let byte_len = bytes.len() as u64;
    let footprint = ChunkFootprint::new(byte_len, byte_len, byte_len, 256);
    let bridge = molgfx_core::ProviderDatasetBridge::new(provider.dataset());
    let data = match bridge.structure_chunk(source, footprint) {
        Ok(value) => value,
        Err(error) => panic!("bridge must retain provider storage: {error}"),
    };
    Fixture {
        data,
        pointer,
        bytes,
        request: ResidencyRequest {
            key: ResidencyKey {
                dataset: DatasetId::new(dataset),
                chunk: ChunkId::new(chunk),
                detail: ResidencyDetail::Atom,
            },
            footprint,
            class: ResidencyClass::Hot,
            priority,
        },
    }
}

pub(super) fn frame_fixture(dataset: u64, chunk: u64, priority: i32, offset: f32) -> Fixture {
    let source_text = CIF
        .replace("1.0 2.0 3.0", &format!("{} 2.0 3.0", 1.0 + offset))
        .replace("4.0 5.0 6.0", &format!("{} 5.0 6.0", 4.0 + offset));
    let structure = match molframe::read_bytes(
        source_text.into_bytes(),
        Some("frame.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((value, _)) => value,
        Err(error) => panic!("frame fixture must parse: {error:?}"),
    };
    let provider = match molframe::FrameChunkProvider::new(
        molframe::DatasetId::new(dataset),
        molframe::ChunkId::new(chunk),
        structure,
        molframe::ModelIndex::new(0),
    ) {
        Ok(value) => value,
        Err(error) => panic!("frame provider must initialize: {error}"),
    };
    let source = match provider.chunk(molframe::ChunkId::new(chunk)) {
        Ok(value) => value,
        Err(error) => panic!("frame chunk must exist: {error}"),
    };
    let pointer = source.positions().as_ptr() as usize;
    let bytes = bytemuck::cast_slice(source.positions()).to_vec();
    let byte_len = bytes.len() as u64;
    let footprint = ChunkFootprint::new(byte_len, byte_len, byte_len, 256);
    let bridge = molgfx_core::ProviderDatasetBridge::new(provider.dataset());
    let data = match bridge.frame_chunk(source, footprint) {
        Ok(value) => value,
        Err(error) => panic!("frame bridge must retain provider storage: {error}"),
    };
    Fixture {
        data,
        pointer,
        bytes,
        request: ResidencyRequest {
            key: ResidencyKey {
                dataset: DatasetId::new(dataset),
                chunk: ChunkId::new(chunk),
                detail: ResidencyDetail::Atom,
            },
            footprint,
            class: ResidencyClass::Hot,
            priority,
        },
    }
}

pub(super) fn request_and_deliver(
    engine: &mut Engine<MockDevice>,
    fixture: Fixture,
    output: &mut ResidencyOutput,
) -> (ResidencyTicket, usize, Vec<u8>) {
    let ticket = match engine.request_chunk_into(fixture.request, output) {
        Ok(value) => value,
        Err(error) => panic!("request must fit: {error}"),
    };
    if let Err(error) = engine.deliver_chunk_into(ticket, fixture.data, output) {
        panic!("delivery must fit: {error}");
    }
    (ticket, fixture.pointer, fixture.bytes)
}

fn retained_pointer(host: &HostWorkingSet, ticket: ResidencyTicket) -> usize {
    let Some(data) = host.payload(ticket.key) else {
        panic!("host payload must remain retained")
    };
    let ChunkPayload::ProviderStructure(structure) = data.payload() else {
        panic!("provider structure payload expected")
    };
    structure.positions().as_ptr() as usize
}

pub(super) fn upload(
    engine: &mut Engine<MockDevice>,
    ticket: ResidencyTicket,
    output: &mut ResidencyOutput,
) {
    if let Err(error) = engine.upload_chunk_into(ticket, output) {
        panic!("upload must be accepted: {error}");
    }
}

pub(super) fn complete(engine: &mut Engine<MockDevice>, output: &mut ResidencyOutput) {
    engine.device.complete_submissions();
    if let Err(error) = engine.poll_chunk_uploads_into(output) {
        panic!("completion must be accepted: {error}");
    }
}

#[test]
fn provider_storage_is_retained_and_exact_bytes_become_resident_only_after_signal() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let source = fixture(u64::from(u32::MAX) + 17, u64::from(u32::MAX) + 31, 1);
    let (ticket, pointer, expected) = request_and_deliver(&mut engine, source, &mut output);
    assert_eq!(retained_pointer(engine.host_working_set(), ticket), pointer);

    upload(&mut engine, ticket, &mut output);
    assert_eq!(
        engine
            .host_working_set()
            .residency()
            .snapshot(ticket.key)
            .phase,
        ResidencyPhase::Uploading
    );
    assert!(engine.resident_structure_chunk(ticket).is_none());
    if let Err(error) = engine.poll_chunk_uploads_into(&mut output) {
        panic!("unresolved poll must remain valid: {error}");
    }
    assert!(engine.resident_structure_chunk(ticket).is_none());

    complete(&mut engine, &mut output);
    let Some(resident) = engine.resident_structure_chunk(ticket) else {
        panic!("completed upload must be resident")
    };
    assert_eq!(resident.local_rows, 2);
    assert_eq!(resident.byte_len, expected.len() as u64);
    assert_eq!(retained_pointer(engine.host_working_set(), ticket), pointer);
    let payloads = match engine.device.log.write_payloads.lock() {
        Ok(value) => value,
        Err(error) => panic!("write log must be available: {error}"),
    };
    let uploaded = payloads.iter().find(|bytes| bytes.starts_with(&expected));
    let Some(uploaded) = uploaded else {
        panic!("one upload must begin with the exact borrowed coordinates")
    };
    assert_eq!(
        uploaded.len(),
        expected.len() + 2 * std::mem::size_of::<f32>()
    );
    assert_eq!(
        &uploaded[expected.len()..],
        bytemuck::cast_slice::<f32, u8>(&[1.70_f32, 1.55_f32]),
        "derived radii follow coordinates without duplicating them"
    );
}

#[test]
fn two_global_chunks_use_distinct_local_ranges_and_stale_work_never_resides() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (first, _, _) = request_and_deliver(&mut engine, fixture(7, 11, 1), &mut output);
    let (second, _, _) = request_and_deliver(&mut engine, fixture(7, 12, 2), &mut output);
    upload(&mut engine, first, &mut output);
    upload(&mut engine, second, &mut output);
    complete(&mut engine, &mut output);
    let first_range = engine
        .resident_structure_chunk(first)
        .map(|value| value.byte_offset);
    let second_range = engine
        .resident_structure_chunk(second)
        .map(|value| value.byte_offset);
    assert!(first_range.is_some());
    assert!(second_range.is_some());
    assert_ne!(first_range, second_range);

    let stale_fixture = fixture(9, 21, 1);
    let (stale, _, _) = request_and_deliver(&mut engine, stale_fixture, &mut output);
    upload(&mut engine, stale, &mut output);
    let replacement = fixture(9, 21, 3);
    let (current, _, _) = request_and_deliver(&mut engine, replacement, &mut output);
    upload(&mut engine, current, &mut output);
    complete(&mut engine, &mut output);
    assert!(engine.resident_structure_chunk(stale).is_none());
    assert!(engine.resident_structure_chunk(current).is_some());
}

#[test]
fn cancellation_pressure_and_device_loss_release_physical_pages() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (cancelled, _, _) = request_and_deliver(&mut engine, fixture(31, 41, 0), &mut output);
    upload(&mut engine, cancelled, &mut output);
    if let Err(error) = engine.cancel_chunk_into(cancelled, &mut output) {
        panic!("current upload must cancel: {error}");
    }
    complete(&mut engine, &mut output);
    assert!(engine.resident_structure_chunk(cancelled).is_none());

    let (lower, _, _) = request_and_deliver(&mut engine, fixture(31, 42, 0), &mut output);
    let (higher, higher_pointer, _) =
        request_and_deliver(&mut engine, fixture(31, 43, 10), &mut output);
    upload(&mut engine, lower, &mut output);
    upload(&mut engine, higher, &mut output);
    complete(&mut engine, &mut output);
    let constrained = ResidencyBudget {
        cpu: 4096,
        staging: 4096,
        gpu_hot: 256,
        gpu_warm: 0,
        in_flight: 4096,
    };
    if let Err(error) = engine.set_chunk_budget_into(constrained, &mut output) {
        panic!("pressure reconciliation must succeed: {error}");
    }
    assert!(engine.resident_structure_chunk(lower).is_none());
    assert!(engine.resident_structure_chunk(higher).is_some());
    assert_eq!(engine.chunk_residency_metrics().arena.resident_bytes, 256);

    let report = match engine.chunk_device_lost_into(&mut output) {
        Ok(value) => value,
        Err(error) => panic!("device loss must preserve host data: {error}"),
    };
    assert_eq!(report.invalidated, 1);
    assert!(engine.resident_structure_chunk(higher).is_none());
    assert_eq!(engine.chunk_residency_metrics().arena.resident_bytes, 0);
    assert_eq!(
        retained_pointer(engine.host_working_set(), higher),
        higher_pointer
    );
    assert_eq!(
        engine
            .host_working_set()
            .residency()
            .snapshot(higher.key)
            .phase,
        ResidencyPhase::ReadyCpu
    );
}

#[test]
fn upload_backpressure_preserves_cpu_payload_for_retry() {
    let mut engine = backpressured_engine();
    let mut output = ResidencyOutput::default();
    let (first, _, _) = request_and_deliver(&mut engine, fixture(71, 81, 1), &mut output);
    let (second, pointer, _) = request_and_deliver(&mut engine, fixture(71, 82, 1), &mut output);
    upload(&mut engine, first, &mut output);
    let rejected = engine.upload_chunk_into(second, &mut output);
    assert!(matches!(
        rejected,
        Err(super::ChunkResidencyError::Upload(
            molgfx_gpu::UploadBackpressure::TicketCapacity
        ))
    ));
    assert_eq!(
        engine
            .host_working_set()
            .residency()
            .snapshot(second.key)
            .phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(retained_pointer(engine.host_working_set(), second), pointer);

    complete(&mut engine, &mut output);
    upload(&mut engine, second, &mut output);
    complete(&mut engine, &mut output);
    assert!(engine.resident_structure_chunk(second).is_some());
}

#[test]
fn stable_frames_do_not_allocate_or_rewrite_resident_chunk_storage() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (ticket, _, _) = request_and_deliver(&mut engine, fixture(51, 61, 1), &mut output);
    upload(&mut engine, ticket, &mut output);
    complete(&mut engine, &mut output);
    let before = engine.chunk_residency_metrics();
    let structure_buffer = match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers
            .iter()
            .find(|(_, label, _)| *label == "resident structure chunks")
            .map(|(id, _, _)| *id),
        Err(_) => None,
    };
    let Some(structure_buffer) = structure_buffer else {
        panic!("structure arena buffer must exist")
    };
    let writes_before = structure_write_count(&engine, structure_buffer);
    let scene = molgfx_core::Scene::new();
    let camera = super::tests::camera();
    for _ in 0..2 {
        if let Err(error) = engine.render(&scene, &camera) {
            panic!("stable frame must render: {error}");
        }
    }
    let after = engine.chunk_residency_metrics();
    assert_eq!(
        after.arena.host_allocation_events,
        before.arena.host_allocation_events
    );
    assert_eq!(
        after.uploads.host_allocation_events,
        before.uploads.host_allocation_events
    );
    assert_eq!(after.tracked_capacity, before.tracked_capacity);
    assert_eq!(
        structure_write_count(&engine, structure_buffer),
        writes_before
    );
}

fn structure_write_count(engine: &Engine<MockDevice>, buffer: u32) -> usize {
    match engine.device.log.writes.lock() {
        Ok(writes) => writes.iter().filter(|(id, _, _, _)| *id == buffer).count(),
        Err(_) => 0,
    }
}

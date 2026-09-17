use super::chunk_residency_tests::{
    complete, engine, fixture, frame_fixture, request_and_deliver, upload,
};
use super::{
    ChunkPlacementId, ChunkPlacementStatus, ChunkRepresentation, Engine, PickEntity,
    StructureChunkPlacement, TrajectoryChunkWindow,
};
use crate::testing::MockDevice;
use molgfx_core::{ChunkId, DatasetId, ResidencyBudget, ResidencyOutput};
use molgfx_math::{Mat4, Rgba8};

#[test]
fn resident_points_render_as_one_indirect_batch_and_pick_global_rows() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let dataset = u64::from(u32::MAX) + 91;
    let chunk = u64::from(u32::MAX) + 93;
    let (ticket, _, _) = request_and_deliver(&mut engine, fixture(dataset, chunk, 1), &mut output);
    upload(&mut engine, ticket, &mut output);
    complete(&mut engine, &mut output);
    let representation = match ChunkRepresentation::points(3.0, Rgba8::opaque(40, 180, 220)) {
        Ok(value) => value,
        Err(error) => panic!("point representation must validate: {error}"),
    };
    let placement_id = ChunkPlacementId::new(7);
    if let Err(error) = engine.set_structure_chunk_placements(&[StructureChunkPlacement {
        id: placement_id,
        ticket,
        model_to_world: Mat4::IDENTITY,
        representation,
    }]) {
        panic!("placement must fit: {error}");
    }
    let scene = molgfx_core::Scene::new();
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("resident chunk must render: {error}");
    }
    assert_eq!(
        engine.structure_chunk_placement_status(placement_id),
        ChunkPlacementStatus::Resident
    );
    let args_id = buffer_id(&engine, "paged indirect command arena");
    assert_eq!(
        indirect_count(&engine, args_id),
        1,
        "one draw covers every resident chunk"
    );
    let buffers_after_warmup = buffer_count(&engine);
    let paged_writes_after_warmup = paged_write_count(&engine);
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("stable paged frame must render: {error}");
    }
    assert_eq!(buffer_count(&engine), buffers_after_warmup);
    assert_eq!(paged_write_count(&engine), paged_writes_after_warmup);
    assert_eq!(indirect_count(&engine, args_id), 2);

    let page = engine.scene_gpu.test_chunk_pick_page(
        DatasetId::new(dataset),
        ChunkId::new(chunk),
        molgfx_core::EntityKind::Atom,
    );
    let Some(page) = page else {
        panic!("resident chunk must own a picking page")
    };
    assert_global_pick(&mut engine, dataset, chunk, page);
    exercise_loss_and_eviction(
        &mut engine,
        &scene,
        &mut output,
        ticket,
        placement_id,
        args_id,
    );
}

#[test]
fn points_and_spacefill_share_one_persistent_indirect_command_arena() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (ticket, _, _) = request_and_deliver(&mut engine, fixture(71, 73, 1), &mut output);
    let (second_ticket, _, _) = request_and_deliver(&mut engine, fixture(79, 83, 2), &mut output);
    upload(&mut engine, ticket, &mut output);
    upload(&mut engine, second_ticket, &mut output);
    complete(&mut engine, &mut output);
    let points = match ChunkRepresentation::points(2.0, Rgba8::opaque(20, 40, 60)) {
        Ok(value) => value,
        Err(error) => panic!("points must validate: {error}"),
    };
    let spacefill = match ChunkRepresentation::spacefill(1.0, Rgba8::opaque(80, 100, 120)) {
        Ok(value) => value,
        Err(error) => panic!("spacefill must validate: {error}"),
    };
    let placements = [
        StructureChunkPlacement {
            id: ChunkPlacementId::new(1),
            ticket,
            model_to_world: Mat4::IDENTITY,
            representation: points,
        },
        StructureChunkPlacement {
            id: ChunkPlacementId::new(2),
            ticket,
            model_to_world: Mat4::IDENTITY,
            representation: spacefill,
        },
        StructureChunkPlacement {
            id: ChunkPlacementId::new(3),
            ticket: second_ticket,
            model_to_world: Mat4::IDENTITY,
            representation: spacefill,
        },
    ];
    if let Err(error) = engine.set_structure_chunk_placements(&placements) {
        panic!("mixed placements must fit: {error}");
    }
    let scene = molgfx_core::Scene::new();
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("mixed paged representations must render: {error}");
    }
    let arena = buffer_id(&engine, "paged indirect command arena");
    assert_eq!(indirect_offsets(&engine, arena), vec![16, 0]);
    let page = engine.scene_gpu.test_chunk_pick_page(
        DatasetId::new(79),
        ChunkId::new(83),
        molgfx_core::EntityKind::Atom,
    );
    let Some(page) = page else {
        panic!("second spacefill chunk must own a picking page")
    };
    assert_global_pick(&mut engine, 79, 83, page);
    let buffers = buffer_count(&engine);
    let writes = paged_write_count(&engine);
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("stable mixed frame must render: {error}");
    }
    assert_eq!(buffer_count(&engine), buffers);
    assert_eq!(paged_write_count(&engine), writes);
    assert_eq!(indirect_offsets(&engine, arena), vec![16, 0, 16, 0]);
}

#[test]
fn spacefill_rejects_invalid_radius_scales() {
    assert!(ChunkRepresentation::spacefill(0.0, Rgba8::opaque(1, 2, 3)).is_err());
    assert!(ChunkRepresentation::spacefill(f32::NAN, Rgba8::opaque(1, 2, 3)).is_err());
}

#[test]
fn provider_frames_interpolate_through_one_bounded_gpu_window() {
    let mut engine = engine();
    let mut output = ResidencyOutput::default();
    let (structure, _, _) = request_and_deliver(&mut engine, fixture(301, 401, 3), &mut output);
    let (start, _, _) =
        request_and_deliver(&mut engine, frame_fixture(302, 501, 2, 0.0), &mut output);
    let (end, _, _) =
        request_and_deliver(&mut engine, frame_fixture(303, 601, 2, 10.0), &mut output);
    for ticket in [structure, start, end] {
        upload(&mut engine, ticket, &mut output);
    }
    complete(&mut engine, &mut output);
    assert!(engine.resident_trajectory_chunk(start).is_some());
    assert!(engine.resident_trajectory_chunk(end).is_some());
    let representation = match ChunkRepresentation::points(3.0, Rgba8::opaque(30, 90, 180)) {
        Ok(value) => value,
        Err(error) => panic!("point representation must validate: {error}"),
    };
    if let Err(error) = engine.set_structure_chunk_placements(&[StructureChunkPlacement {
        id: ChunkPlacementId::new(81),
        ticket: structure,
        model_to_world: Mat4::IDENTITY,
        representation,
    }]) {
        panic!("structure placement must fit: {error}");
    }
    let window = match TrajectoryChunkWindow::new(structure, start, end, 0.25) {
        Ok(value) => value,
        Err(error) => panic!("trajectory window must validate: {error}"),
    };
    if let Err(error) = engine.set_trajectory_chunk_windows(&[window]) {
        panic!("trajectory window must fit: {error}");
    }
    let scene = molgfx_core::Scene::new();
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("paged trajectory must render: {error}");
    }
    let frame_buffer = buffer_id(&engine, "resident trajectory frames");
    let display_buffer = buffer_id(&engine, "resident display coordinates");
    assert_ne!(frame_buffer, u32::MAX);
    assert_ne!(display_buffer, u32::MAX);
    let writes_before = buffer_write_count(&engine, frame_buffer);
    let advanced = match TrajectoryChunkWindow::new(structure, start, end, 0.75) {
        Ok(value) => value,
        Err(error) => panic!("advanced window must validate: {error}"),
    };
    if let Err(error) = engine.set_trajectory_chunk_windows(&[advanced]) {
        panic!("time-only update must fit: {error}");
    }
    if let Err(error) = engine.render(&scene, &super::tests::camera()) {
        panic!("advanced trajectory must render: {error}");
    }
    assert_eq!(buffer_write_count(&engine, frame_buffer), writes_before);
}

fn exercise_loss_and_eviction(
    engine: &mut Engine<MockDevice>,
    scene: &molgfx_core::Scene,
    output: &mut ResidencyOutput,
    ticket: molgfx_core::ResidencyTicket,
    placement: ChunkPlacementId,
    args: u32,
) {
    if let Err(error) = engine.chunk_device_lost_into(output) {
        panic!("device loss must preserve the CPU payload: {error}");
    }
    render_absent(engine, scene, placement, args, 2, "device-lost");
    upload(engine, ticket, output);
    complete(engine, output);
    if let Err(error) = engine.render(scene, &super::tests::camera()) {
        panic!("re-uploaded placement must rejoin the batch: {error}");
    }
    assert_eq!(
        engine.structure_chunk_placement_status(placement),
        ChunkPlacementStatus::Resident
    );
    assert_eq!(indirect_count(engine, args), 3);
    if let Err(error) = engine.set_chunk_budget_into(
        ResidencyBudget {
            cpu: 4096,
            staging: 4096,
            gpu_hot: 0,
            gpu_warm: 0,
            in_flight: 4096,
        },
        output,
    ) {
        panic!("eviction must reconcile: {error}");
    }
    render_absent(engine, scene, placement, args, 3, "evicted");
}

fn render_absent(
    engine: &mut Engine<MockDevice>,
    scene: &molgfx_core::Scene,
    placement: ChunkPlacementId,
    args: u32,
    draws: usize,
    state: &str,
) {
    if let Err(error) = engine.render(scene, &super::tests::camera()) {
        panic!("{state} placement must stop drawing cleanly: {error}");
    }
    assert_eq!(
        engine.structure_chunk_placement_status(placement),
        ChunkPlacementStatus::NotResident
    );
    assert_eq!(indirect_count(engine, args), draws);
}

fn buffer_id(engine: &Engine<MockDevice>, label: &'static str) -> u32 {
    match engine.device.log.buffers.lock() {
        Ok(buffers) => buffers
            .iter()
            .find(|(_, candidate, _)| *candidate == label)
            .map_or(u32::MAX, |(id, _, _)| *id),
        Err(_) => u32::MAX,
    }
}

fn buffer_count(engine: &Engine<MockDevice>) -> usize {
    engine
        .device
        .log
        .buffers
        .lock()
        .map_or(0, |value| value.len())
}

fn indirect_count(engine: &Engine<MockDevice>, buffer: u32) -> usize {
    engine.device.log.indirect_draws.lock().map_or(0, |draws| {
        draws.iter().filter(|(id, _)| *id == buffer).count()
    })
}

fn indirect_offsets(engine: &Engine<MockDevice>, buffer: u32) -> Vec<u64> {
    engine.device.log.indirect_draws.lock().map_or_else(
        |_| Vec::new(),
        |draws| {
            draws
                .iter()
                .filter_map(|(id, offset)| (*id == buffer).then_some(*offset))
                .collect()
        },
    )
}

fn paged_write_count(engine: &Engine<MockDevice>) -> usize {
    let placement = buffer_id(engine, "paged chunk placements");
    let coordinates = buffer_id(engine, "resident structure chunks");
    let clusters = buffer_id(engine, "resident structure chunk clusters");
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes
            .iter()
            .filter(|(id, _, _, _)| [placement, coordinates, clusters].contains(id))
            .count()
    })
}

fn buffer_write_count(engine: &Engine<MockDevice>, buffer: u32) -> usize {
    engine.device.log.writes.lock().map_or(0, |writes| {
        writes.iter().filter(|(id, _, _, _)| *id == buffer).count()
    })
}

fn assert_global_pick(engine: &mut Engine<MockDevice>, dataset: u64, chunk: u64, page: u32) {
    if let Ok(mut value) = engine.device.log.pick_resident_page.lock() {
        *value = page;
    }
    if let Ok(mut value) = engine.device.log.pick_local_row.lock() {
        *value = 1;
    }
    let picked = match engine.pick(0, 0) {
        Ok(Some(value)) => value,
        Ok(None) => panic!("mock point must resolve"),
        Err(error) => panic!("global pick must resolve: {error}"),
    };
    let PickEntity::Structure(identity) = picked.entity else {
        panic!("molecular pick expected")
    };
    assert_eq!(identity.dataset(), DatasetId::new(dataset));
    assert_eq!(identity.chunk(), ChunkId::new(chunk));
    assert_eq!(identity.row().get(), 1);
}

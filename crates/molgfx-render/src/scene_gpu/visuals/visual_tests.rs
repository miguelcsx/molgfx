use super::{VisualBase, VisualConfigInput, VisualFallback, result_word_count, styled_config};
use crate::testing::{Locked, MockDevice};
use molgfx_core::{
    AtomSelection, Representation, RepresentationKind, RepresentationTarget, Scene,
    VisualProgramBuilder, VisualStyle,
};

#[test]
fn visual_config_carries_time_property_offsets_and_displacement_bound() {
    let mut builder = VisualProgramBuilder::new();
    let offset = builder
        .vector([3.0, 0.0, 0.0])
        .unwrap_or_else(|error| panic!("offset should build: {error}"));
    builder
        .set_position_offset(offset, 1.5)
        .unwrap_or_else(|error| panic!("displacement should build: {error}"));
    let style = VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("program should build: {error}")),
    );
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    let offsets = [1, 1_000_001, 2_000_001, 3_000_001];
    let config = styled_config(&VisualConfigInput {
        program: style.program(),
        style: &style,
        base: VisualBase::representation(&representation),
        entity_count: 1_000_000,
        result_count: 1_000_000,
        property_offsets: offsets,
        attribute_layouts: [0; 4],
        property_end_offsets: [0; 4],
        property_alphas: [0.0; 4],
        time_seconds: 0.75,
        track: molgfx_core::TrackClock::default(),
        program_offset: 17,
        parameter_offset: 32,
        state_offset: 1,
    });

    assert_eq!(config.counts[1], 1_000_000);
    assert_eq!(config.property_offsets, offsets);
    assert_eq!(config.presentation[0].to_bits(), 0.75f32.to_bits());
    assert_eq!(config.presentation[1].to_bits(), 1.5f32.to_bits());
    assert_eq!(config.counts[2], 0);
    assert_eq!(config.arena_offsets[0], 17);
    assert_eq!(config.arena_offsets[1], 32);
    assert_eq!(config.result_layout[0], 0);
    assert_eq!(config.uniform_offset[..3], [1.5, 0.0, 0.0]);
    assert_eq!(result_word_count(config.result_layout), 0);
}

#[test]
fn visual_config_counts_only_fragment_stage_instructions_for_pipeline_routing() {
    let mut builder = VisualProgramBuilder::new();
    let normal = builder
        .normal()
        .unwrap_or_else(|error| panic!("normal should build: {error}"));
    let axis = builder
        .vector([0.0, 0.0, 1.0])
        .unwrap_or_else(|error| panic!("axis should build: {error}"));
    let roughness = builder
        .dot(normal, axis)
        .unwrap_or_else(|error| panic!("dot should build: {error}"));
    builder
        .set_roughness(roughness)
        .unwrap_or_else(|error| panic!("roughness should build: {error}"));
    let style = VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("program should build: {error}")),
    );
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    let config = styled_config(&VisualConfigInput {
        program: style.program(),
        style: &style,
        base: VisualBase::representation(&representation),
        entity_count: 1_000_000,
        result_count: 1_000_000,
        property_offsets: [0; 4],
        attribute_layouts: [0; 4],
        property_end_offsets: [0; 4],
        property_alphas: [0.0; 4],
        time_seconds: 0.0,
        track: molgfx_core::TrackClock::default(),
        program_offset: 0,
        parameter_offset: 0,
        state_offset: 1,
    });

    assert_eq!(
        config.counts[2],
        u32::try_from(style.program().fragment_instruction_count())
            .unwrap_or_else(|error| panic!("fragment count should fit: {error}"))
    );
    assert_eq!(config.counts[2], 2);
    assert_eq!(config.result_layout[0], 0);
    assert_eq!(result_word_count(config.result_layout), 0);
}

#[test]
fn disabled_visual_buffers_are_initialized_exactly_once() {
    let device = MockDevice::default();
    let queue = device.queue();
    let mut fallback = VisualFallback::new(&device)
        .unwrap_or_else(|error| panic!("fallback should allocate: {error}"));

    assert!(fallback.sync(&queue));
    assert!(!fallback.sync(&queue));
    let writes = device.log.writes.locked();
    assert_eq!(writes.len(), 6);
}

#[test]
fn visual_config_narrows_the_track_clock_next_to_the_time_uniform() {
    let mut builder = VisualProgramBuilder::new();
    let phase = builder
        .track_phase()
        .unwrap_or_else(|error| panic!("track phase should build: {error}"));
    builder
        .set_opacity(phase)
        .unwrap_or_else(|error| panic!("opacity should build: {error}"));
    let style = VisualStyle::new(
        builder
            .finish()
            .unwrap_or_else(|error| panic!("program should build: {error}")),
    );
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    let representation = Representation::new(
        RepresentationTarget::Selection(selection),
        RepresentationKind::Spacefill,
    );
    let config = styled_config(&VisualConfigInput {
        program: style.program(),
        style: &style,
        base: VisualBase::representation(&representation),
        entity_count: 1,
        result_count: 1,
        property_offsets: [0; 4],
        attribute_layouts: [0; 4],
        property_end_offsets: [0; 4],
        property_alphas: [0.0; 4],
        time_seconds: 0.5,
        track: molgfx_core::TrackClock {
            phase: 0.25,
            display_time: 25.0,
        },
        program_offset: 0,
        parameter_offset: 0,
        state_offset: 1,
    });

    assert_eq!(config.presentation[0].to_bits(), 0.5f32.to_bits());
    assert_eq!(config.presentation[2].to_bits(), 0.25f32.to_bits());
    assert_eq!(config.presentation[3].to_bits(), 25.0f32.to_bits());
    assert_eq!(config.base_color[3].to_bits(), 0.25f32.to_bits());
}

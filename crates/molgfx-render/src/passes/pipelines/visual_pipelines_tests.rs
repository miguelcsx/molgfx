use super::{VISUAL_FRAGMENT_CONSTANT, VISUAL_PROGRAM_CONSTANT, VisualPipelineSet, constants};
use crate::scene_gpu::SlotShading;
use crate::testing::MockDevice;

#[test]
fn built_in_entity_and_fragment_styles_select_distinct_pipelines() {
    let pipelines = VisualPipelineSet::<MockDevice>::new(11, 22, 33);

    assert_eq!(*pipelines.get(SlotShading::default()), 11);
    assert_eq!(*pipelines.get(SlotShading::default().with_visual(true)), 22);
    assert_eq!(
        *pipelines.get(
            SlotShading::default()
                .with_visual(true)
                .with_fragment_visual(true),
        ),
        33
    );
}

#[test]
fn a_resolved_pipeline_replaces_only_the_fragment_stage() {
    let built_in = 11;
    let entity = 22;
    let fragment = 33;
    let specialized = 44;
    let pipelines = VisualPipelineSet::<MockDevice>::new(built_in, entity, fragment);
    let fragment_visual = SlotShading::default()
        .with_visual(true)
        .with_fragment_visual(true);
    // The fragment stage takes the generated pipeline.
    assert_eq!(*pipelines.select(fragment_visual, Some(&specialized)), 44);
    // No resolved pipeline, or any other shading, falls back to the
    // unconditional selection: an entity or built-in draw can never be routed
    // onto generated code by a stray specialized handle.
    assert_eq!(*pipelines.select(fragment_visual, None), 33);
    assert_eq!(
        *pipelines.select(SlotShading::default().with_visual(true), Some(&specialized)),
        22
    );
    assert_eq!(
        *pipelines.select(SlotShading::default(), Some(&specialized)),
        11
    );
    // A draw without a resolved pipeline keeps the interpreted unit, so a set
    // behaves exactly as it did before specialization existed.
    assert_eq!(*pipelines.select(fragment_visual, None), 33);
}

#[test]
fn visual_specialization_constants_enable_only_the_requested_stage() {
    assert_eq!(
        constants(false),
        [
            (VISUAL_PROGRAM_CONSTANT, 1.0),
            (VISUAL_FRAGMENT_CONSTANT, 0.0),
        ]
    );
    assert_eq!(
        constants(true),
        [
            (VISUAL_PROGRAM_CONSTANT, 1.0),
            (VISUAL_FRAGMENT_CONSTANT, 1.0),
        ]
    );
}

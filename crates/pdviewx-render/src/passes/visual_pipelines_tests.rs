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

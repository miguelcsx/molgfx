use super::VisualProgramTable;
use crate::testing::MockDevice;
use molgfx_core::{AtomSelection, Representation, Scene, VisualProgramBuilder, VisualStyle};

fn color_style(color: [f32; 4]) -> VisualStyle {
    let mut builder = VisualProgramBuilder::new();
    let color = builder
        .color(color)
        .unwrap_or_else(|error| panic!("color should build: {error}"));
    VisualStyle::new(
        builder
            .finish_color(color)
            .unwrap_or_else(|error| panic!("style should build: {error}")),
    )
}

#[test]
fn identical_programs_share_one_stable_instruction_range() {
    let first = color_style([1.0, 0.0, 0.0, 1.0]);
    let second = first.clone();
    let mut scene = Scene::new();
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(selection, Representation::spacefill().visual(first.clone()))
        .unwrap_or_else(|error| panic!("first representation should build: {error}"));
    scene
        .represent(selection, Representation::points().visual(second))
        .unwrap_or_else(|error| panic!("second representation should build: {error}"));
    let device = MockDevice::default();
    let queue = device.queue();
    let mut table = VisualProgramTable::<MockDevice>::new();

    table
        .sync(&device, &queue, &scene)
        .unwrap_or_else(|error| panic!("program table should sync: {error}"));

    assert_eq!(table.programs.len(), 1);
    assert_eq!(table.offset(first.program()), Some(0));
    assert_eq!(
        table.instructions.len(),
        first.program().instructions().len()
    );
}

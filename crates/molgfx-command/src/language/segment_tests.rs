use crate::{Command, Program};
use molgfx_scene::{Color, DataSource, SegmentStyle, SegmentationId, SegmentationSpec};

fn segmentation() -> SegmentationSpec {
    SegmentationSpec {
        presentation: molgfx_scene::SegmentationPresentation::Surface,
        source: DataSource::new("labels"),
        dimensions: [2, 2, 2],
        voxel_to_world: [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
        styles: vec![SegmentStyle {
            label: u32::MAX,
            color: Color::rgb(4, 5, 6),
            opacity: 0.5,
            visible: true,
        }],
    }
}

#[test]
fn segmentation_declarations_round_trip_through_text_and_serde() {
    let command = Command::Segment {
        segmentation: segmentation(),
    };
    let program = Program::parse(&command.to_string()).expect("segment text");
    assert_eq!(program.statements()[0].command, command);
    let json = serde_json::to_string(&command).expect("command JSON");
    assert_eq!(
        serde_json::from_str::<Command>(&json).expect("command"),
        command
    );
}

#[test]
fn style_commands_preserve_full_generational_identity_and_empty_styles() {
    for styles in [segmentation().styles, Vec::new()] {
        let command = Command::SegmentStyle {
            id: SegmentationId {
                index: u64::MAX,
                generation: 12,
            },
            styles,
        };
        let program = Program::parse(&command.to_string()).expect("style text");
        assert_eq!(program.statements()[0].command, command);
        let json = serde_json::to_string(&command).expect("command JSON");
        assert_eq!(
            serde_json::from_str::<Command>(&json).expect("command"),
            command
        );
    }
}

#[test]
fn malformed_segmentation_commands_are_rejected() {
    for text in [
        "segment",
        "segment {}",
        "segment style",
        "segment style 1 []",
        "segment style 1:x []",
        "segment style 1:0 {}",
        "segment style 1:0 [{\"label\":1}]",
        "segment style 1:0 [] extra",
    ] {
        assert!(Program::parse(text).is_err(), "{text}");
    }
}

#[test]
fn segmentation_json_is_not_split_at_commas_or_quoted_semicolons() {
    let mut spec = segmentation();
    spec.source = DataSource::new("labels;with,delimiters");
    let command = Command::Segment { segmentation: spec };
    let text = format!("{command};segment style 1:0 []");
    assert_eq!(
        Program::parse(&text)
            .expect("two commands")
            .statements()
            .len(),
        2
    );
}

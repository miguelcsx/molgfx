use super::parse;
use molgfx_scene::VolumePresentation;

const AFFINE: &str = "[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]";

#[test]
fn explicit_volume_commands_preserve_all_presentation_variants() {
    let forms = [
        (
            "iso",
            r#"{"isovalue":1.25,"color":[49,104,142,255],"opacity":0.6,"style":{"kind":"solid"}}"#,
        ),
        (
            "direct",
            r#"{"transfer":[{"value":0,"color":[0,0,0,255],"opacity":0},{"value":1,"color":[255,255,255,255],"opacity":1}],"opacity_scale":2,"step_scale":0.65}"#,
        ),
        (
            "slice",
            r#"{"point":[0,0,0],"normal":[0,0,1],"ramp":"viridis","domain":[0,1]}"#,
        ),
    ];
    for (kind, settings) in forms {
        let command = format!("source density dims [2,2,2] affine {AFFINE} {kind} {settings}");
        let volume = parse(&command).expect("explicit volume command");
        assert_eq!(volume.dimensions, [2, 2, 2]);
        assert_eq!(volume.voxel_to_world[0].to_bits(), 1.0_f32.to_bits());
        assert!(
            matches!(
                (&volume.presentations[0], kind),
                (VolumePresentation::Isosurface { isovalue, .. }, "iso") if isovalue.to_bits() == 1.25_f32.to_bits()
            ) || matches!(
                (&volume.presentations[0], kind),
                (VolumePresentation::Direct { transfer, .. }, "direct") if transfer[1].value.to_bits() == 1.0_f32.to_bits()
            ) || matches!(
                (&volume.presentations[0], kind),
                (VolumePresentation::Slice { normal, .. }, "slice") if normal.map(f32::to_bits) == [0.0_f32.to_bits(), 0.0_f32.to_bits(), 1.0_f32.to_bits()]
            )
        );
        let printed = serde_json::to_string(&volume).expect("volume JSON");
        assert_eq!(parse(&printed).expect("canonical volume command"), volume);
    }
}

#[test]
fn malformed_volume_presentation_is_rejected() {
    let command = format!("source density dims [2,2,2] affine {AFFINE} direct {{\"transfer\":[]}}");
    assert!(parse(&command).is_err());
}

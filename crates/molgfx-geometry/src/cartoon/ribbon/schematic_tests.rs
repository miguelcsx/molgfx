use super::*;
use molgfx_core::{EntityId, EntityKind};

#[test]
fn schematic_stencils_keep_original_atom_and_oxygen_rows_and_leave_helix_fits_intact() {
    let entities = [4, 8, 12].map(|row| EntityId::pack(EntityKind::Atom, row).expect("atom").0);
    let mut recipes = [
        RibbonDeformation {
            controls: [4, 4, 8, 12],
            parameter: [0.5, 0.0, 0.0, 0.0],
        },
        RibbonDeformation {
            controls: [4, 8, 4, 12],
            parameter: [0.5, 0.0, 0.0, f32::from_bits(6)],
        },
    ];
    let helix = recipes[1];
    let mut properties = vec![7.0];
    prepare(
        &mut recipes,
        &entities,
        &[
            SecondaryStructure::Strand,
            SecondaryStructure::BetaBridge,
            SecondaryStructure::Coil,
        ],
        &[5, 9, 13],
        &mut properties,
        true,
    )
    .expect("stencils");
    assert_eq!(recipes[0].controls, [1, 1, 6, 11]);
    assert_eq!(recipes[0].parameter[3].to_bits(), 14);
    assert_eq!(recipes[1], helix);
    assert_eq!(
        properties[1..6]
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [4, 4, 8, 1, 5]
    );
    assert_eq!(
        properties[6..11]
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [4, 8, 12, 0, 9]
    );
}

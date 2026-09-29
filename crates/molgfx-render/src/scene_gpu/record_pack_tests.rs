use super::hide_bonded_line_atoms;
use molgfx_core::{AtomGpu, BondGpu};

#[test]
fn line_crosses_remain_only_for_atoms_without_selected_bonds() {
    let mut atoms = [
        AtomGpu {
            radius: 1.0,
            ..AtomGpu::default()
        },
        AtomGpu {
            radius: 1.5,
            ..AtomGpu::default()
        },
        AtomGpu {
            radius: 2.0,
            ..AtomGpu::default()
        },
    ];
    let bonds = [BondGpu {
        atom_a: 0,
        atom_b: 1,
        ..BondGpu::default()
    }];

    hide_bonded_line_atoms(&mut atoms, &bonds);

    assert_eq!(atoms[0].radius.to_bits(), 0.0f32.to_bits());
    assert_eq!(atoms[1].radius.to_bits(), 0.0f32.to_bits());
    assert_eq!(atoms[2].radius.to_bits(), 2.0f32.to_bits());
}

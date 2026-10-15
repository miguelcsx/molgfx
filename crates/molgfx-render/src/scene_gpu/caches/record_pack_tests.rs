use super::hide_bonded_line_atoms;
use molgfx_core::{AtomFlags, AtomGpu, BondGpu};

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

    assert!(atoms[0].flags.contains(AtomFlags::HIDE_MARKER));
    assert!(atoms[1].flags.contains(AtomFlags::HIDE_MARKER));
    assert!(!atoms[2].flags.contains(AtomFlags::HIDE_MARKER));
    assert_eq!(atoms[0].radius.to_bits(), 1.0f32.to_bits());
    assert_eq!(atoms[1].radius.to_bits(), 1.5f32.to_bits());
    assert_eq!(atoms[2].radius.to_bits(), 2.0f32.to_bits());
}

use super::*;
use std::mem::{align_of, offset_of, size_of};

#[test]
fn an_atom_record_is_exactly_thirty_two_bytes_at_sixteen_byte_alignment() {
    assert_eq!(size_of::<AtomGpu>(), 32);
    assert_eq!(align_of::<AtomGpu>(), 16);
}

#[test]
fn atom_record_fields_sit_at_their_contracted_offsets() {
    assert_eq!(offset_of!(AtomGpu, position), 0);
    assert_eq!(offset_of!(AtomGpu, radius), 12);
    assert_eq!(offset_of!(AtomGpu, color), 16);
    assert_eq!(offset_of!(AtomGpu, element), 20);
    assert_eq!(offset_of!(AtomGpu, flags), 22);
    assert_eq!(offset_of!(AtomGpu, entity_id), 24);
    assert_eq!(offset_of!(AtomGpu, semantic), 28);
}

#[test]
fn a_bond_record_is_exactly_twelve_bytes_at_four_byte_alignment() {
    assert_eq!(size_of::<BondGpu>(), 12);
    assert_eq!(align_of::<BondGpu>(), 4);
    assert_eq!(offset_of!(BondGpu, atom_a), 0);
    assert_eq!(offset_of!(BondGpu, atom_b), 4);
    assert_eq!(offset_of!(BondGpu, radius), 8);
}

#[test]
fn indirect_draw_arguments_match_the_sixteen_byte_wire_format() {
    assert_eq!(size_of::<DrawIndirectArgs>(), 16);
    assert_eq!(offset_of!(DrawIndirectArgs, vertex_count), 0);
    assert_eq!(offset_of!(DrawIndirectArgs, instance_count), 4);
    assert_eq!(offset_of!(DrawIndirectArgs, first_vertex), 8);
    assert_eq!(offset_of!(DrawIndirectArgs, first_instance), 12);
}

#[test]
fn a_slice_of_atom_records_casts_to_bytes_and_back() {
    let atoms = vec![AtomGpu::default(); 3];
    let bytes: &[u8] = bytemuck::cast_slice(&atoms);
    assert_eq!(bytes.len(), 96);
    let back: &[AtomGpu] = bytemuck::cast_slice(bytes);
    assert_eq!(back.len(), 3);
}

#[test]
fn entity_ids_round_trip_every_kind_and_boundary_index() {
    for kind in [
        EntityKind::Atom,
        EntityKind::Bond,
        EntityKind::Edge,
        EntityKind::Label,
    ] {
        for index in [0u32, 1, 99_999, EntityId::MAX_INDEX] {
            let id = EntityId::pack(kind, index);
            let Some((k, i)) = id.unpack() else {
                panic!("packed id must unpack")
            };
            assert_eq!(k, kind);
            assert_eq!(i, index);
        }
    }
}

#[test]
fn the_empty_entity_sentinel_unpacks_to_nothing() {
    assert!(EntityId::NONE.unpack().is_none());
}

#[test]
fn aromatic_bonds_keep_their_flag_and_radius_through_packing() {
    let plain = BondGpu::new(1, 2, 0.2, false);
    assert!(!plain.is_aromatic());
    assert!((plain.draw_radius() - 0.2).abs() < 1e-6);

    let aromatic = BondGpu::new(3, 4, 0.2, true);
    assert!(aromatic.is_aromatic());
    assert!((aromatic.draw_radius() - 0.2).abs() < 1e-6);

    // A zero input radius must not erase the sign bit.
    let degenerate = BondGpu::new(5, 6, 0.0, true);
    assert!(degenerate.is_aromatic());
    assert!(degenerate.draw_radius() > 0.0);
}

#[test]
fn flag_sets_combine_and_test_like_sets() {
    let flags = AtomFlags::VISIBLE.union(AtomFlags::FOCUSED);
    assert!(flags.contains(AtomFlags::VISIBLE));
    assert!(flags.contains(AtomFlags::FOCUSED));
    assert!(!flags.contains(AtomFlags::GHOSTED));
    let cleared = flags.difference(AtomFlags::FOCUSED);
    assert!(!cleared.contains(AtomFlags::FOCUSED));
    assert!(cleared.contains(AtomFlags::VISIBLE));
}

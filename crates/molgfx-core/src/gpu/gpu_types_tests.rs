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
fn a_bond_record_is_exactly_sixteen_bytes_at_four_byte_alignment() {
    assert_eq!(size_of::<BondGpu>(), 16);
    assert_eq!(align_of::<BondGpu>(), 4);
    assert_eq!(offset_of!(BondGpu, atom_a), 0);
    assert_eq!(offset_of!(BondGpu, atom_b), 4);
    assert_eq!(offset_of!(BondGpu, radius), 8);
    assert_eq!(offset_of!(BondGpu, entity_id), 12);
}

#[test]
fn an_interaction_record_is_six_aligned_gpu_lanes() {
    assert_eq!(size_of::<InteractionGpu>(), 96);
    assert_eq!(align_of::<InteractionGpu>(), 16);
    assert_eq!(offset_of!(InteractionGpu, start_width), 0);
    assert_eq!(offset_of!(InteractionGpu, end_period), 16);
    assert_eq!(offset_of!(InteractionGpu, color), 32);
    assert_eq!(offset_of!(InteractionGpu, metadata), 48);
    assert_eq!(offset_of!(InteractionGpu, style), 64);
    assert_eq!(offset_of!(InteractionGpu, animation), 80);
}

#[test]
fn a_primitive_record_is_seven_aligned_gpu_lanes() {
    assert_eq!(size_of::<PrimitiveGpu>(), 112);
    assert_eq!(align_of::<PrimitiveGpu>(), 16);
    assert_eq!(offset_of!(PrimitiveGpu, center_radius), 0);
    assert_eq!(offset_of!(PrimitiveGpu, orientation), 16);
    assert_eq!(offset_of!(PrimitiveGpu, size_opacity), 32);
    assert_eq!(offset_of!(PrimitiveGpu, inverse_primary), 48);
    assert_eq!(offset_of!(PrimitiveGpu, inverse_cross), 64);
    assert_eq!(offset_of!(PrimitiveGpu, color), 80);
    assert_eq!(offset_of!(PrimitiveGpu, metadata), 96);
}

#[test]
fn a_particle_motion_record_is_four_aligned_gpu_lanes() {
    assert_eq!(size_of::<ParticleMotionGpu>(), 64);
    assert_eq!(align_of::<ParticleMotionGpu>(), 16);
    assert_eq!(offset_of!(ParticleMotionGpu, velocity_step), 0);
    assert_eq!(offset_of!(ParticleMotionGpu, minimum), 16);
    assert_eq!(offset_of!(ParticleMotionGpu, maximum), 32);
    assert_eq!(offset_of!(ParticleMotionGpu, metadata), 48);
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
        EntityKind::Primitive,
        EntityKind::Mesh,
        EntityKind::LigandPoseBatch,
        EntityKind::Guide,
        EntityKind::DynamicBond,
    ] {
        for index in [0u32, 1, 99_999, EntityId::MAX_INDEX] {
            let id =
                EntityId::pack(kind, u64::from(index)).unwrap_or_else(|error| panic!("{error}"));
            let Some((k, i)) = id.unpack() else {
                panic!("packed id must unpack")
            };
            assert_eq!(k, kind);
            assert_eq!(i, index);
        }
    }
}

#[test]
fn ligand_pose_batches_and_primitives_never_alias_at_the_same_row() {
    let primitive =
        EntityId::pack(EntityKind::Primitive, 42).unwrap_or_else(|error| panic!("{error}"));
    let batch =
        EntityId::pack(EntityKind::LigandPoseBatch, 42).unwrap_or_else(|error| panic!("{error}"));

    assert_ne!(primitive, batch);
    assert_eq!(primitive.unpack(), Some((EntityKind::Primitive, 42)));
    assert_eq!(batch.unpack(), Some((EntityKind::LigandPoseBatch, 42)));
}

#[test]
fn guides_and_interactions_never_alias_at_the_same_row() {
    let interaction =
        EntityId::pack(EntityKind::Edge, 42).unwrap_or_else(|error| panic!("{error}"));
    let guide = EntityId::pack(EntityKind::Guide, 42).unwrap_or_else(|error| panic!("{error}"));

    assert_ne!(interaction, guide);
    assert_eq!(interaction.unpack(), Some((EntityKind::Edge, 42)));
    assert_eq!(guide.unpack(), Some((EntityKind::Guide, 42)));
}

#[test]
fn the_shared_glyph_record_keeps_guide_and_interaction_tags_distinct() {
    let owner = crate::StructureHandle(crate::handle::RawHandle::new_for_test(0, 0));
    let start = crate::InteractionAnchor::world(molgfx_math::Vec3::ZERO)
        .unwrap_or_else(|error| panic!("{error}"));
    let end = crate::InteractionAnchor::world(molgfx_math::Vec3::X)
        .unwrap_or_else(|error| panic!("{error}"));
    let geometry =
        crate::InteractionGeometry::new(1.0, None).unwrap_or_else(|error| panic!("{error}"));
    let edge = crate::InteractionEdge::new(
        owner,
        start,
        end,
        crate::InteractionKind::HydrogenBond,
        geometry,
        "test",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let guide = crate::Guide::new(
        owner,
        molgfx_math::Vec3::ZERO,
        molgfx_math::Vec3::Y,
        crate::GuideStyle::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let edge_gpu = InteractionGpu::new(&edge, 0, 0).unwrap_or_else(|error| panic!("{error}"));
    let guide_gpu =
        InteractionGpu::from_guide(&guide, 0, 0).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        EntityId(edge_gpu.metadata[0]).unpack(),
        Some((EntityKind::Edge, 0))
    );
    assert_eq!(
        EntityId(guide_gpu.metadata[0]).unpack(),
        Some((EntityKind::Guide, 0))
    );
}

#[test]
fn the_empty_entity_sentinel_unpacks_to_nothing() {
    assert!(EntityId::NONE.unpack().is_none());
}

#[test]
fn entity_ids_reject_every_index_above_the_attachment_limit() {
    let just_above = u64::from(EntityId::MAX_INDEX) + 1;
    let above_u32 = u64::from(u32::MAX) + 1;

    for index in [just_above, u64::from(u32::MAX), above_u32] {
        let error = EntityId::pack(EntityKind::Atom, index)
            .expect_err("an out-of-range local row must be rejected");
        assert_eq!(error.index(), index);
    }
}

#[test]
fn aromatic_bonds_keep_their_flag_and_radius_through_packing() {
    let plain_id = EntityId::pack(EntityKind::Bond, 4).unwrap_or_else(|error| panic!("{error}"));
    let plain = BondGpu::new(1, 2, 0.2, false, plain_id);
    assert!(!plain.is_aromatic());
    assert!((plain.draw_radius() - 0.2).abs() < 1e-6);

    let aromatic_id = EntityId::pack(EntityKind::Bond, 5).unwrap_or_else(|error| panic!("{error}"));
    let aromatic = BondGpu::new(3, 4, 0.2, true, aromatic_id);
    assert!(aromatic.is_aromatic());
    assert!((aromatic.draw_radius() - 0.2).abs() < 1e-6);

    // A zero input radius must not erase the sign bit.
    let degenerate_id =
        EntityId::pack(EntityKind::Bond, 6).unwrap_or_else(|error| panic!("{error}"));
    let degenerate = BondGpu::new(5, 6, 0.0, true, degenerate_id);
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

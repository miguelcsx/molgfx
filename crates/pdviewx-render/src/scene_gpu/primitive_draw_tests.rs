use super::*;

fn row(active: bool) -> PackedPrimitive {
    let mut motion = ParticleMotionGpu::default();
    motion.metadata[0] = u32::from(active);
    PackedPrimitive::new(PrimitiveGpu::default(), motion)
}

fn shaped(family: u32, shape: u32, alpha: f32) -> PackedPrimitive {
    let mut record = PrimitiveGpu::default();
    record.metadata[2] = family;
    record.metadata[3] = shape;
    record.color[3] = alpha;
    PackedPrimitive::new(record, ParticleMotionGpu::default())
}

#[test]
fn static_tables_omit_per_primitive_auxiliary_columns() {
    let mut rows = [row(false), row(false)];
    let mut records = Vec::new();
    let mut previous = Vec::new();
    let mut motion = Vec::new();
    let mut groups = Vec::new();

    regroup(
        &mut rows,
        &mut records,
        &mut previous,
        &mut motion,
        &mut groups,
        false,
    );

    assert_eq!(records.len(), 2);
    assert!(previous.is_empty());
    assert!(motion.is_empty());
    assert_eq!(records[0].inverse_cross[3].to_bits(), 0.0_f32.to_bits());
}

#[test]
fn moving_tables_keep_lockstep_auxiliary_columns() {
    let mut rows = [row(false), row(true)];
    let mut records = Vec::new();
    let mut previous = Vec::new();
    let mut motion = Vec::new();
    let mut groups = Vec::new();

    regroup(
        &mut rows,
        &mut records,
        &mut previous,
        &mut motion,
        &mut groups,
        true,
    );

    assert_eq!(previous.len(), records.len());
    assert_eq!(motion.len(), records.len());
    assert_eq!(records[1].inverse_cross[3].to_bits(), 1.0_f32.to_bits());
}

#[test]
fn regroup_separates_polygon_sides_particle_shapes_and_transparency_ranges() {
    let mut rows = [
        shaped(FAMILY_POLYGON, 4, 1.0),
        shaped(FAMILY_PARTICLE, 4, 0.5),
        shaped(FAMILY_POLYGON, 1, 1.0),
        shaped(FAMILY_PARTICLE, 0, 1.0),
        shaped(FAMILY_POLYGON, 5, 1.0),
    ];
    let mut records = Vec::new();
    let mut previous = Vec::new();
    let mut motion = Vec::new();
    let mut groups = Vec::new();

    regroup(
        &mut rows,
        &mut records,
        &mut previous,
        &mut motion,
        &mut groups,
        false,
    );

    assert_eq!(
        groups,
        vec![
            PrimitiveDrawGroup {
                family: FAMILY_POLYGON,
                shape: POLYGON_PENTAGON,
                translucent: false,
                first: 0,
                len: 2,
            },
            PrimitiveDrawGroup {
                family: FAMILY_POLYGON,
                shape: POLYGON_HEXAGON,
                translucent: false,
                first: 2,
                len: 1,
            },
            PrimitiveDrawGroup {
                family: FAMILY_PARTICLE,
                shape: 0,
                translucent: false,
                first: 3,
                len: 1,
            },
            PrimitiveDrawGroup {
                family: FAMILY_PARTICLE,
                shape: 4,
                translucent: true,
                first: 4,
                len: 1,
            },
        ]
    );
}

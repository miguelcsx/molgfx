use super::{
    atom_cull_entries, bond_cull_entries, quality_entries, relation_cull_entries,
    relation_resolve_entries, representation_entries, storage_counts, validate_storage_limit,
    visual_cull_entries,
};
use molgfx_gpu::{GpuError, ShaderStages};

const PORTABLE_COMPUTE_LIMIT: u32 = 8;
const PORTABLE_FRAGMENT_LIMIT: u32 = 8;

#[test]
fn representation_storage_bindings_fit_every_portable_stage() {
    let counts = storage_counts(&representation_entries());

    assert_eq!(counts.fragment, PORTABLE_FRAGMENT_LIMIT);
    assert!(counts.vertex <= PORTABLE_FRAGMENT_LIMIT);
    assert!(counts.fragment <= PORTABLE_FRAGMENT_LIMIT);
    assert!(counts.compute <= PORTABLE_FRAGMENT_LIMIT);
}

#[test]
fn surface_motion_reads_previous_coordinates_in_the_fragment_stage_without_fragment_bonds() {
    let entries = representation_entries();
    assert!(entries.iter().any(|entry| {
        entry.binding == 13
            && entry.visibility == ShaderStages::VERTEX.union(ShaderStages::FRAGMENT)
    }));
    assert!(
        entries
            .iter()
            .any(|entry| { entry.binding == 3 && entry.visibility == ShaderStages::VERTEX })
    );
}

#[test]
fn quality_storage_bindings_fit_the_portable_fragment_limit() {
    let counts = storage_counts(&quality_entries());

    assert_eq!(counts.fragment, PORTABLE_FRAGMENT_LIMIT);
    assert!(counts.fragment <= PORTABLE_FRAGMENT_LIMIT);
    assert_eq!(counts.vertex, 0);
    assert_eq!(counts.compute, 0);
}

#[test]
fn compute_culling_uses_but_does_not_exceed_the_portable_limit() {
    for counts in [
        storage_counts(&atom_cull_entries()),
        storage_counts(&bond_cull_entries()),
        storage_counts(&visual_cull_entries()),
    ] {
        assert!(counts.compute <= PORTABLE_COMPUTE_LIMIT);
        assert_eq!(counts.fragment, 0);
        assert_eq!(counts.vertex, 0);
    }
}

#[test]
fn dynamic_relation_resolution_uses_only_its_homogeneous_sources() {
    let counts = storage_counts(&relation_resolve_entries());

    assert_eq!(
        counts.compute, 6,
        "the maximal rigid/rigid stream needs two timeline sources per endpoint"
    );
    assert!(counts.compute <= PORTABLE_COMPUTE_LIMIT);
    assert_eq!(counts.vertex, 0);
    assert_eq!(counts.fragment, 0);
}

#[test]
fn relation_culling_stays_well_below_the_portable_storage_limit() {
    let counts = storage_counts(&relation_cull_entries());

    assert_eq!(counts.compute, 8);
    assert!(counts.compute <= PORTABLE_COMPUTE_LIMIT);
    assert_eq!(counts.vertex, 0);
    assert_eq!(counts.fragment, 0);
}

#[test]
fn layout_contract_returns_a_typed_error_before_backend_validation() {
    let error = validate_storage_limit("test representation", &representation_entries(), 7);

    assert!(matches!(
        error,
        Err(GpuError::LimitExceeded {
            resource: "test representation",
            limit: 7
        })
    ));
}

#[test]
fn fragment_program_is_uniform_and_not_counted_as_storage() {
    let entries = representation_entries();
    let fragment_program = entries.iter().find(|entry| entry.binding == 17);

    assert!(fragment_program.is_some_and(|entry| {
        entry.visibility == ShaderStages::FRAGMENT
            && matches!(entry.ty, molgfx_gpu::BindingType::Uniform)
    }));
    assert!(entries.iter().all(|entry| entry.binding != 18));
}

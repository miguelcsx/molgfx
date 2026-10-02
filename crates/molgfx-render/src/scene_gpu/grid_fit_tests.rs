use super::*;
use crate::scene_gpu::detail::GridLimits;

#[test]
fn surface_grid_uses_angstrom_spacing_until_the_dimension_cap() {
    use crate::scene_gpu::detail::INTERACTIVE_SURFACE_DIMENSION as CAP;

    assert_eq!(axis_cells(10.0, 0.25, CAP), 41);
    assert_eq!(axis_cells(100.0, 0.25, CAP), CAP);
    // A larger limit keeps the requested spacing instead of coarsening.
    assert_eq!(axis_cells(100.0, 0.25, 1024), 401);
}

#[test]
fn surface_grid_always_has_an_interpolatable_cell() {
    assert_eq!(axis_cells(0.0, 0.25, 192), 2);
    assert_eq!(axis_cells(0.1, 0.25, 192), 2);
}

#[test]
fn a_field_over_the_voxel_budget_is_coarsened_until_it_fits_and_says_so() {
    let extent = Vec3::splat(100.0);
    let open = fit_grid(
        extent,
        GridLimits {
            spacing: 0.25,
            max_dimension: 1024,
            max_cells: u32::MAX,
        },
    );
    assert!(!open.memory_limited);
    assert!((open.cell - 0.25).abs() < f32::EPSILON);

    let budget = 8 * 1024 * 1024;
    let fitted = fit_grid(
        extent,
        GridLimits {
            spacing: 0.25,
            max_dimension: 1024,
            max_cells: budget,
        },
    );
    let voxels: u64 = fitted
        .dimensions
        .iter()
        .map(|&axis| u64::from(axis))
        .product();
    assert!(fitted.memory_limited);
    assert!(voxels <= u64::from(budget), "{voxels} voxels");
    assert!(
        voxels * 10 > u64::from(budget) * 6,
        "the budget is used, not undershot: {voxels}"
    );
    assert!(fitted.cell > 0.25);
}

#[test]
fn a_field_inside_the_budget_is_untouched() {
    let fitted = fit_grid(
        Vec3::splat(20.0),
        GridLimits {
            spacing: 0.25,
            max_dimension: 1024,
            max_cells: 8 * 1024 * 1024,
        },
    );
    assert!(!fitted.memory_limited);
    assert!((fitted.cell - 0.25).abs() < f32::EPSILON);
}

#[test]
fn a_budget_too_small_for_any_field_still_terminates() {
    let fitted = fit_grid(
        Vec3::splat(50.0),
        GridLimits {
            spacing: 0.25,
            max_dimension: 192,
            max_cells: 1,
        },
    );
    assert!(fitted.memory_limited);
    assert_eq!(fitted.dimensions, [2, 2, 2]);
}

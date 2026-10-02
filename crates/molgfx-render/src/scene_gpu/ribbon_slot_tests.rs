use super::*;

#[test]
fn a_ribbon_twice_the_budget_is_sampled_half_as_finely() {
    assert_eq!(reduced_steps(8, MAX_RIBBON_VERTICES * 2), 4);
}

#[test]
fn a_ribbon_just_over_the_budget_still_loses_a_step() {
    assert_eq!(reduced_steps(8, MAX_RIBBON_VERTICES + 1), 7);
}

#[test]
fn the_reduction_never_goes_below_the_coarsest_sampling() {
    assert_eq!(
        reduced_steps(8, MAX_RIBBON_VERTICES * 100),
        MIN_RIBBON_STEPS
    );
}

#[test]
fn the_reduction_always_lowers_the_step_limit_so_the_fit_ends() {
    for steps in MIN_RIBBON_STEPS + 1..=u8::MAX {
        for vertices in [MAX_RIBBON_VERTICES + 1, MAX_RIBBON_VERTICES * 3] {
            assert!(reduced_steps(steps, vertices) < steps);
        }
    }
}

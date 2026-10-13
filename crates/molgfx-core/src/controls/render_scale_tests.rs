use super::ScaleController;

/// Scale steps are discrete constants, so any drift beyond rounding is a bug.
fn assert_scale(actual: f32, expected: f32, context: &str) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "scale {actual} should be {expected}: {context}"
    );
}

#[test]
fn a_gpu_that_keeps_up_never_loses_resolution() {
    let mut c = ScaleController::new();
    for _ in 0..1000 {
        c.observe(false);
    }
    assert_scale(c.scale(), 1.0, "");
}

#[test]
fn repeated_skips_step_the_scale_down_one_notch_at_a_time() {
    let mut c = ScaleController::new();
    for _ in 0..3 {
        c.observe(true);
    }
    assert_scale(c.scale(), 0.75, "");
    for _ in 0..3 {
        c.observe(true);
    }
    assert_scale(c.scale(), 0.5, "");
}

#[test]
fn an_occasional_skip_does_not_lower_quality() {
    let mut c = ScaleController::new();
    for i in 0..200 {
        c.observe(i % 10 == 0);
    }
    assert_scale(c.scale(), 1.0, "");
}

#[test]
fn the_scale_never_goes_below_the_floor() {
    let mut c = ScaleController::new();
    for _ in 0..1000 {
        c.observe(true);
    }
    assert_scale(c.scale(), 0.35, "");
}

#[test]
fn a_long_clean_run_earns_the_resolution_back() {
    let mut c = ScaleController::new();
    for _ in 0..6 {
        c.observe(true);
    }
    assert_scale(c.scale(), 0.5, "");
    for _ in 0..40 {
        c.observe(false);
    }
    assert_scale(c.scale(), 0.75, "");
    for _ in 0..40 {
        c.observe(false);
    }
    assert_scale(c.scale(), 1.0, "");
}

#[test]
fn full_quality_can_be_locked() {
    let mut c = ScaleController::new();
    for _ in 0..6 {
        c.observe(true);
    }
    c.lock_full(true);
    assert_scale(c.scale(), 1.0, "");
    for _ in 0..100 {
        c.observe(true);
    }
    assert_scale(c.scale(), 1.0, "");
}

#[test]
fn a_frame_that_fits_the_budget_stays_at_full_resolution() {
    let mut c = ScaleController::new();
    c.set_budget(Some(8.0));
    c.set_cost(Some(7.5));
    assert_scale(c.scale(), 1.0, "");
    c.set_cost(None);
    assert_scale(c.scale(), 1.0, "unknown cost means full resolution");
}

#[test]
fn a_frame_over_budget_is_drawn_smaller_in_proportion_to_the_overrun() {
    let mut c = ScaleController::new();
    c.set_budget(Some(8.0));
    c.set_cost(Some(12.0)); // 1.5x: wanted scale 0.82, the next step down is 0.75
    assert_scale(c.scale(), 0.75, "");
    c.set_cost(Some(24.0)); // 3x: wanted 0.58 -> 0.5
    assert_scale(c.scale(), 0.5, "");
    c.set_cost(Some(200.0)); // far over: the floor
    assert_scale(c.scale(), 0.35, "");
}

#[test]
fn the_plan_and_the_skips_both_apply_and_the_cheaper_wins() {
    let mut c = ScaleController::new();
    c.set_budget(Some(8.0));
    c.set_cost(Some(12.0)); // plan: 0.75
    for _ in 0..3 {
        c.observe(true);
    }
    assert_scale(c.scale(), 0.75, "skips said 0.75 as well");
    for _ in 0..3 {
        c.observe(true);
    }
    assert_scale(
        c.scale(),
        0.5,
        "the GPU is still behind: lower than planned",
    );
}

#[test]
fn a_pinned_scale_ignores_the_plan() {
    let mut c = ScaleController::new();
    c.set_budget(Some(8.0));
    c.set_cost(Some(100.0));
    c.pin(Some(0.9));
    assert_scale(c.scale(), 0.9, "");
}

#[test]
fn nonsense_costs_and_budgets_are_ignored() {
    let mut c = ScaleController::new();
    c.set_budget(Some(f32::NAN));
    c.set_cost(Some(-3.0));
    assert_scale(c.scale(), 1.0, "");
}

#[test]
fn the_plan_is_ignored_while_the_measured_picture_is_not_the_one_drawn() {
    let mut c = ScaleController::new();
    c.set_budget(Some(8.0));
    c.set_cost(Some(200.0));
    assert_scale(c.scale(), 0.35, "");
    c.set_plan_off(true);
    assert_scale(
        c.scale(),
        1.0,
        "its surfaces are set aside: it is cheaper than measured",
    );
    c.set_plan_off(false);
    assert_scale(c.scale(), 0.35, "");
}

#[test]
fn a_gpu_that_made_the_skips_halve_the_resolution_is_lagging() {
    let mut c = ScaleController::new();
    assert!(!c.is_lagging());
    for _ in 0..3 {
        c.observe(true);
    }
    assert!(!c.is_lagging(), "one notch down is not yet lagging");
    for _ in 0..3 {
        c.observe(true);
    }
    assert!(c.is_lagging(), "down to 0.5");
}

fn priced(cost: f32, budget: f32) -> ScaleController {
    let mut c = ScaleController::new();
    c.set_budget(Some(budget));
    c.set_cost(Some(cost));
    c
}

#[test]
fn a_cheap_or_unmeasured_picture_is_never_simplified_for_a_gesture() {
    let mut c = ScaleController::new();
    assert!(!c.wants_aside(false, 10_000), "unmeasured");
    c.set_budget(Some(8.0));
    c.set_cost(Some(10.0));
    assert!(
        !c.wants_aside(false, 10_000),
        "within one and a half frames"
    );
    assert!(!priced(20.0, 16.7).wants_aside(true, 10_000));
}

#[test]
fn a_tick_of_the_wheel_never_swaps_a_surface_unless_the_frame_is_hopeless() {
    // dearer than the display allows, but only by half again and a bit
    let c = priced(30.0, 16.7);
    assert!(!c.wants_aside(true, 0));
    assert!(
        !c.wants_aside(true, 1_000),
        "a long zoom is still only a zoom"
    );
    // far too dear: a zoom swaps once it has gone on a moment, not on the first tick
    let c = priced(80.0, 16.7);
    assert!(!c.wants_aside(true, 0));
    assert!(c.wants_aside(true, 200));
}

#[test]
fn a_drag_swaps_at_once_when_far_too_dear_and_after_a_moment_when_somewhat_dear() {
    let far = priced(80.0, 16.7);
    assert!(far.wants_aside(false, 0));
    let some = priced(30.0, 16.7);
    assert!(!some.wants_aside(false, 50));
    assert!(some.wants_aside(false, 200));
}

#[test]
fn a_gpu_that_really_fell_behind_swaps_whatever_the_gesture() {
    let mut c = ScaleController::new();
    for _ in 0..6 {
        c.observe(true);
    }
    assert!(c.wants_aside(true, 0));
    assert!(c.wants_aside(false, 0));
}

#[test]
fn a_crowd_of_molecules_is_simplified_when_a_gesture_has_gone_on_and_the_frame_misses_the_display()
{
    let c = priced(13.0, 8.3);
    assert!(!c.wants_crowd_aside(0), "not on the first instant");
    assert!(c.wants_crowd_aside(200));
    assert!(!priced(6.0, 8.3).wants_crowd_aside(500), "it fits");
    assert!(
        !priced(8.0, 4.0).wants_crowd_aside(500),
        "cheaper than 8 ms is left alone"
    );
    let unmeasured = ScaleController::new();
    assert!(!unmeasured.wants_crowd_aside(500));
}

use super::{AdaptiveQuality, AdaptiveQualityConfig, QualityTier};

/// A 60 Hz budget in nanoseconds, chosen so the band edges are exact.
const BUDGET_NS: u64 = 16_666_666;
/// Exactly `5/4` of the budget: the top of the tolerated band.
const OVER_BAND_NS: u64 = 20_833_332;
/// Exactly `3/4` of the budget: the bottom of the tolerated band.
const UNDER_BAND_NS: u64 = 12_499_999;
/// Frames of sustained headroom before a tier steps up.
const UP_FRAMES: u32 = 48;

fn controller() -> AdaptiveQuality {
    AdaptiveQuality::new(AdaptiveQualityConfig::interactive(60), false)
}

#[test]
fn a_frame_time_inside_the_band_never_moves_the_tier() {
    let mut quality = controller();
    for frame in 0..2_000u32 {
        // Alternate across both edges every frame: the most adversarial
        // signal a steady 60 Hz presenter can actually produce.
        let nanos = if frame % 2 == 0 {
            UNDER_BAND_NS
        } else {
            OVER_BAND_NS
        };
        assert_eq!(
            quality.observe(nanos),
            QualityTier::Reduced,
            "frame {frame}"
        );
    }
}

#[test]
fn sustained_overrun_steps_down_and_sustained_headroom_steps_back_up() {
    let mut quality = controller();
    for _ in 0..200 {
        quality.observe(2 * BUDGET_NS);
    }
    assert_eq!(quality.tier(), QualityTier::Minimal);
    for _ in 0..400 {
        quality.observe(BUDGET_NS / 2);
    }
    assert_eq!(quality.tier(), QualityTier::High);
}

#[test]
fn a_single_slow_frame_is_reset_away_and_never_moves_the_tier() {
    let mut quality = controller();
    let start = quality.tier();
    // Build the step-up streak to one frame short of the requirement.
    for _ in 0..UP_FRAMES - 1 {
        quality.observe(BUDGET_NS / 2);
    }
    assert_eq!(quality.tier(), start);
    // One outlier cancels the streak: the tier needs 48 *consecutive* frames
    // of headroom, so this frame neither steps up nor steps down.
    quality.observe(10 * BUDGET_NS);
    assert_eq!(quality.tier(), start);
    // Sustained headroom does eventually step up, which is what makes the
    // outlier above meaningful rather than a permanently stuck controller. The
    // smoothed time has to decay out of the overrun band first, so this takes
    // more than `UP_FRAMES` frames.
    for _ in 0..(UP_FRAMES * 8) {
        quality.observe(BUDGET_NS / 2);
    }
    assert!(
        quality.tier() > start,
        "sustained headroom must step the tier up"
    );
}

#[test]
fn a_publication_controller_holds_a_constant_tier_under_any_load() {
    let mut quality = AdaptiveQuality::new(AdaptiveQualityConfig::publication(), true);
    assert!(!quality.enabled());
    for _ in 0..500 {
        quality.observe(50 * BUDGET_NS);
    }
    assert_eq!(quality.tier(), QualityTier::Standard);
    assert_eq!(quality.smoothed_ns(), 0);
    for _ in 0..500 {
        quality.observe(0);
    }
    assert_eq!(quality.tier(), QualityTier::Standard);
}

#[test]
fn entering_publication_during_an_adaptive_run_holds_the_standard_tier() {
    let mut quality = controller();
    for _ in 0..200 {
        quality.observe(2 * BUDGET_NS);
    }
    assert_eq!(quality.tier(), QualityTier::Minimal);
    quality.set_publication(true);
    assert!(!quality.enabled());
    assert_eq!(quality.tier(), QualityTier::Standard);
    for _ in 0..200 {
        quality.observe(8 * BUDGET_NS);
    }
    assert_eq!(quality.tier(), QualityTier::Standard);
}

#[test]
fn a_disabled_controller_never_adapts_but_still_reports_its_tier() {
    let mut quality = AdaptiveQuality::new(
        AdaptiveQualityConfig {
            target_fps: 60,
            enabled: false,
        },
        false,
    );
    assert!(!quality.enabled());
    let start = quality.tier();
    for _ in 0..500 {
        quality.observe(10 * BUDGET_NS);
    }
    assert_eq!(quality.tier(), start, "a disabled loop never moves");
}

#[test]
fn low_tiers_keep_at_least_one_temporal_sample() {
    for tier in QualityTier::ALL {
        assert!(tier.temporal_samples() >= 1, "{tier:?}");
        assert!(tier.image_samples() >= 1, "{tier:?}");
        assert!(tier.surface_grid_spacing() > 0.0, "{tier:?}");
    }
}

#[test]
fn tier_knobs_are_monotone_in_cost() {
    let mut previous = QualityTier::ALL[0];
    for tier in QualityTier::ALL.into_iter().skip(1) {
        assert!(tier > previous);
        assert!(tier.surface_grid_spacing() < previous.surface_grid_spacing());
        assert!(tier.temporal_samples() > previous.temporal_samples());
        assert!(tier.image_samples() > previous.image_samples());
        previous = tier;
    }
}

#[test]
fn the_tier_ladder_walks_one_step_at_a_time_and_stops_at_both_ends() {
    assert_eq!(QualityTier::Minimal.cheaper(), None);
    assert_eq!(QualityTier::High.richer(), None);
    assert_eq!(QualityTier::default(), QualityTier::Standard);
    assert_eq!(QualityTier::Standard.cheaper(), Some(QualityTier::Reduced));
    assert_eq!(QualityTier::Standard.richer(), Some(QualityTier::High));
    assert_eq!(QualityTier::Minimal.richer(), Some(QualityTier::Reduced));
    assert_eq!(QualityTier::Reduced.cheaper(), Some(QualityTier::Minimal));
}

#[test]
fn an_impossible_refresh_rate_is_clamped_instead_of_dividing_by_zero() {
    let mut quality = AdaptiveQuality::new(
        AdaptiveQualityConfig {
            target_fps: 0,
            enabled: true,
        },
        false,
    );
    // A 1 Hz budget: 2x overrun is still just one second per frame.
    for _ in 0..200 {
        quality.observe(2_000_000_000);
    }
    assert_eq!(quality.tier(), QualityTier::Minimal);
    assert_eq!(quality.smoothed_ns(), 2_000_000_000);
}

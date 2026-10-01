use super::*;

#[test]
fn refresh_rate_profiles_clamp_impossible_targets() {
    assert_eq!(adaptive(0).target_fps, 1);
    assert_eq!(adaptive(144).target_fps, 144);
    assert_eq!(adaptive(u16::MAX).target_fps, 1_000);
    assert_eq!(highest_fixed(0).target_fps, 1);
    assert_eq!(highest_fixed(144).target_fps, 144);
    assert_eq!(highest_fixed(u16::MAX).target_fps, 1_000);
}

#[test]
fn depth_cues_validate_order_and_strength() {
    let cue = DepthCue::new(4.0, 18.0, 0.35).expect("valid depth cue");
    assert_eq!(cue.near_distance().to_bits(), 4.0_f32.to_bits());
    assert_eq!(cue.far_distance().to_bits(), 18.0_f32.to_bits());
    assert_eq!(cue.strength().to_bits(), 0.35_f32.to_bits());
    assert!(DepthCue::new(18.0, 4.0, 0.35).is_err());
    assert!(DepthCue::new(4.0, 18.0, 1.1).is_err());
}

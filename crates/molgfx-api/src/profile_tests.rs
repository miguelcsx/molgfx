use super::*;

#[test]
fn adaptive_profiles_clamp_impossible_refresh_rates() {
    assert_eq!(adaptive(0).target_fps, 1);
    assert_eq!(adaptive(144).target_fps, 144);
    assert_eq!(adaptive(u16::MAX).target_fps, 1_000);
}

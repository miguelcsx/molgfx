//! Detail budgets the quality tier hands to scene synchronization.
//!
//! A tier changes how finely geometry is sampled, not what is drawn. The
//! values are gathered here so a tier move is one comparable value: any slot
//! whose stored detail differs re-derives exactly the geometry that reads it
//! and nothing else.

/// Surface field spacing, in ångström, of the finest tier.
pub(crate) const FINEST_SURFACE_SPACING: f32 = 0.25;
/// Ribbon samples per trace interval at the richest tier.
pub(crate) const RICHEST_RIBBON_STEPS: u8 = 8;

/// Sampling density for the geometry that scales with a quality tier.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct TierDetail {
    /// Surface field grid spacing in ångström.
    pub(crate) surface_spacing: f32,
    /// Maximum ribbon samples per trace interval.
    pub(crate) ribbon_steps: u8,
}

impl Default for TierDetail {
    /// The richest detail: what publication and offscreen rendering use.
    fn default() -> Self {
        Self {
            surface_spacing: FINEST_SURFACE_SPACING,
            ribbon_steps: RICHEST_RIBBON_STEPS,
        }
    }
}

//! Detail budgets the quality tier hands to scene synchronization.
//!
//! A tier changes how finely geometry is sampled, not what is drawn. The
//! values are gathered here so a tier move is one comparable value: any slot
//! whose stored detail differs re-derives exactly the geometry that reads it
//! and nothing else.

/// Surface field spacing, in ångström, of the finest tier.
pub(crate) const FINEST_SURFACE_SPACING: f32 = 0.25;
/// Longest surface-field axis, in cells, an interactive tier will allocate.
///
/// Beyond this a field is coarsened rather than grown, which keeps the memory
/// and generation cost of a large structure's surface bounded while it is
/// being navigated.
pub(crate) const INTERACTIVE_SURFACE_DIMENSION: u32 = 192;
/// Ribbon samples per trace interval at the richest tier.
pub(crate) const RICHEST_RIBBON_STEPS: u8 = 8;

/// Sampling density for the geometry that scales with a quality tier.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct TierDetail {
    /// Surface field grid spacing in ångström.
    pub(crate) surface_spacing: f32,
    /// Maximum ribbon samples per trace interval.
    pub(crate) ribbon_steps: u8,
    /// Whether projected-radius LOD may replace analytic geometry.
    pub(crate) lod_enabled: bool,
    /// Longest surface-field axis in cells; `u32::MAX` leaves it to the device.
    pub(crate) surface_max_dimension: u32,
}

impl TierDetail {
    /// The longest field axis this tier may use on a device whose largest 3-D
    /// texture axis is `device_limit`.
    ///
    /// A field that cannot be sampled at the requested spacing within this
    /// limit is coarsened, and the render reports the spacing it actually used.
    #[must_use]
    pub(crate) fn surface_dimension_limit(self, device_limit: u32) -> u32 {
        self.surface_max_dimension.min(device_limit).max(2)
    }
}

impl Default for TierDetail {
    /// The richest detail: what publication and offscreen rendering use.
    fn default() -> Self {
        Self {
            surface_spacing: FINEST_SURFACE_SPACING,
            ribbon_steps: RICHEST_RIBBON_STEPS,
            lod_enabled: false,
            surface_max_dimension: u32::MAX,
        }
    }
}

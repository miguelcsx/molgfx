//! Closed-loop adaptive quality: sustained frame time in, quality tier out.
//!
//! The loop is deliberately slow and hysteretic. A tier is a presentation
//! contract, not a per-frame guess: a texture pool rebuild, a surface grid
//! rebuild and a temporal history reset all follow a change, so reacting to
//! one noisy frame would cost more than it saves. A tier therefore moves only
//! after a sustained run of frames past the band edge, and any move resets the
//! measurement window so the new tier is judged on its own evidence.
//!
//! Publication rendering never adapts. Converged output must be reproducible,
//! so the controller holds a constant tier whenever the caller requests it.
//!
//! Frame times arrive as host wall-clock nanoseconds covering one complete
//! `Engine::render` call, submission and presentation included. That is the
//! only per-frame duration the normal path can observe: the presentation path
//! records no timestamp queries, and a query readback would charge a blocking
//! map to every interactive frame.

/// Frame-time smoothing weight: the previous average keeps seven eighths.
const EMA_KEEP: u64 = 7;
/// Divisor matching [`EMA_KEEP`].
const EMA_TOTAL: u64 = EMA_KEEP + 1;
/// Sustained overrun frames before a tier steps down.
const DOWN_FRAMES: u32 = 12;
/// Sustained headroom frames before a tier steps up.
const UP_FRAMES: u32 = 48;
/// Overrun band: the average must exceed `5/4` of the target budget.
const OVERRUN_NUMERATOR: u64 = 5;
/// Denominator of the overrun band.
const OVERRUN_DENOMINATOR: u64 = 4;
/// Headroom band: the average must fall below `3/4` of the target budget.
const HEADROOM_NUMERATOR: u64 = 3;
/// Denominator of the headroom band.
const HEADROOM_DENOMINATOR: u64 = 4;

/// Quality tiers from cheapest to richest.
///
/// The order is the control axis: [`AdaptiveQuality`] steps one tier at a
/// time, so a tier carries no meaning beyond its position in this list.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum QualityTier {
    /// Lowest cost: coarsest surface grids, narrowest sample budgets.
    Minimal,
    /// Below the standard tier; still progressive, never below one sample.
    Reduced,
    /// The default interactive tier, and the tier every non-adaptive
    /// presentation holds.
    #[default]
    Standard,
    /// Full sample budgets for converged interactive output.
    High,
}

impl QualityTier {
    /// Every tier, cheapest first.
    pub const ALL: [Self; 4] = [Self::Minimal, Self::Reduced, Self::Standard, Self::High];

    const fn index(self) -> usize {
        match self {
            Self::Minimal => 0,
            Self::Reduced => 1,
            Self::Standard => 2,
            Self::High => 3,
        }
    }

    /// The next cheaper tier, or `None` at the floor.
    #[must_use]
    pub const fn cheaper(self) -> Option<Self> {
        match self {
            Self::Minimal => None,
            Self::Reduced => Some(Self::Minimal),
            Self::Standard => Some(Self::Reduced),
            Self::High => Some(Self::Standard),
        }
    }

    /// The next richer tier, or `None` at the ceiling.
    #[must_use]
    pub const fn richer(self) -> Option<Self> {
        match self {
            Self::Minimal => Some(Self::Reduced),
            Self::Reduced => Some(Self::Standard),
            Self::Standard => Some(Self::High),
            Self::High => None,
        }
    }

    /// Surface field grid spacing in Ångström for this tier.
    #[must_use]
    pub const fn surface_grid_spacing(self) -> f32 {
        [0.75, 0.5, 0.375, 0.25][self.index()]
    }

    /// Temporal accumulation budget for this tier, in samples.
    #[must_use]
    pub const fn temporal_samples(self) -> u8 {
        [4, 8, 16, 64][self.index()]
    }

    /// Off-screen image sample count for this tier.
    #[must_use]
    pub const fn image_samples(self) -> u32 {
        [4, 16, 32, 64][self.index()]
    }
}

/// Caller policy for the adaptive loop.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdaptiveQualityConfig {
    /// Frame rate the loop steers toward, in frames per second.
    pub target_fps: u16,
    /// Whether the loop may move a tier. Publication callers clear this so
    /// converged output stays reproducible.
    pub enabled: bool,
}

impl AdaptiveQualityConfig {
    /// An adapting loop steering toward `target_fps`.
    #[must_use]
    pub const fn interactive(target_fps: u16) -> Self {
        Self {
            target_fps,
            enabled: true,
        }
    }

    /// A fixed-tier loop for deterministic publication output.
    #[must_use]
    pub const fn publication() -> Self {
        Self {
            target_fps: 1,
            enabled: false,
        }
    }

    /// The frame budget in nanoseconds, clamped to a representable rate.
    const fn budget_ns(self) -> u64 {
        let fps = if self.target_fps == 0 {
            1
        } else if self.target_fps > 1_000 {
            1_000
        } else {
            self.target_fps
        };
        1_000_000_000 / fps as u64
    }
}

impl Default for AdaptiveQualityConfig {
    fn default() -> Self {
        Self::interactive(60)
    }
}

/// Exponentially smoothed frame time with hysteresis over [`QualityTier`].
#[derive(Clone, Copy, Debug)]
pub struct AdaptiveQuality {
    requested: bool,
    publication: bool,
    target_fps: u16,
    target_ns: u64,
    ema_ns: u64,
    tier: QualityTier,
    overrun_frames: u32,
    headroom_frames: u32,
}

impl AdaptiveQuality {
    /// Builds a controller at the tier one render mode starts from.
    ///
    /// `publication` is the engine's own determinism switch: the cinematic path
    /// and off-screen publication never adapt regardless of policy, and they
    /// start at [`QualityTier::Standard`] — the tier it holds for every frame.
    /// An interactive path starts at [`QualityTier::Reduced`], the tier whose
    /// sampling matches the realtime presets the engine shipped before the
    /// loop existed, and the loop raises it once there is measured headroom.
    #[must_use]
    pub const fn new(config: AdaptiveQualityConfig, publication: bool) -> Self {
        Self {
            requested: config.enabled,
            publication,
            target_fps: config.target_fps,
            target_ns: config.budget_ns(),
            ema_ns: 0,
            tier: if publication {
                QualityTier::Standard
            } else {
                QualityTier::Reduced
            },
            overrun_frames: 0,
            headroom_frames: 0,
        }
    }

    /// The frame rate the loop steers toward.
    #[must_use]
    pub const fn target_fps(&self) -> u16 {
        self.target_fps
    }

    /// Whether the loop is allowed to move a tier.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.requested && !self.publication
    }

    /// The tier every frame of this instant presents at.
    #[must_use]
    pub const fn tier(&self) -> QualityTier {
        self.tier
    }

    /// The smoothed frame time in nanoseconds; zero before the first frame.
    #[must_use]
    pub const fn smoothed_ns(&self) -> u64 {
        self.ema_ns
    }

    /// Marks the engine as running the deterministic publication path.
    ///
    /// Entering publication holds the tier constant from that frame on.
    pub const fn set_publication(&mut self, publication: bool) {
        if self.publication != publication {
            self.publication = publication;
            self.tier = if publication {
                QualityTier::Standard
            } else {
                QualityTier::Reduced
            };
            self.reset_window();
        }
    }

    /// Feeds one completed frame's wall-clock duration and returns the tier
    /// the next frame presents at.
    pub const fn observe(&mut self, frame_ns: u64) -> QualityTier {
        if !self.enabled() {
            return self.tier;
        }
        self.ema_ns = if self.ema_ns == 0 {
            frame_ns
        } else {
            self.ema_ns
                .saturating_mul(EMA_KEEP)
                .saturating_add(frame_ns)
                / EMA_TOTAL
        };
        let budget = self.target_ns;
        if self.ema_ns.saturating_mul(OVERRUN_DENOMINATOR)
            > budget.saturating_mul(OVERRUN_NUMERATOR)
        {
            self.overrun_frames = self.overrun_frames.saturating_add(1);
            self.headroom_frames = 0;
        } else if self.ema_ns.saturating_mul(HEADROOM_DENOMINATOR)
            < budget.saturating_mul(HEADROOM_NUMERATOR)
        {
            self.headroom_frames = self.headroom_frames.saturating_add(1);
            self.overrun_frames = 0;
        } else {
            self.overrun_frames = 0;
            self.headroom_frames = 0;
        }
        if self.overrun_frames >= DOWN_FRAMES {
            self.step(self.tier.cheaper());
        } else if self.headroom_frames >= UP_FRAMES {
            self.step(self.tier.richer());
        }
        self.tier
    }

    const fn step(&mut self, next: Option<QualityTier>) {
        match next {
            Some(tier) => self.tier = tier,
            None => self.overrun_frames = 0,
        }
        self.reset_window();
    }

    const fn reset_window(&mut self) {
        self.ema_ns = 0;
        self.overrun_frames = 0;
        self.headroom_frames = 0;
    }
}

impl Default for AdaptiveQuality {
    fn default() -> Self {
        Self::new(AdaptiveQualityConfig::default(), false)
    }
}

#[cfg(test)]
#[path = "adaptive_tests.rs"]
mod tests;

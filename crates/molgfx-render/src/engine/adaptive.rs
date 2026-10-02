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
//! Native rendering feeds the controller CPU frame duration — the elapsed time
//! of one `render` call, synchronization, recording and submission included.
//! Queue submission is asynchronous on native, so host time is the honest
//! per-frame CPU cost; a fence sample there would double-count the same frame.
//! Browser rendering instead samples elapsed time from tracked submission to
//! fence completion, observed on a later frame poll: submission returns
//! immediately in the browser, so measuring the `render` call would classify
//! queued GPU work as free. Neither source measures GPU execution time; the
//! browser sample additionally includes host callback-dispatch latency, and
//! exact device time requires timestamp queries through the profiling path.
//! Completion samples never read pixels or wait synchronously.

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
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTier {
    /// Lowest cost: coarsest surface grids, narrowest sample budgets.
    Minimal,
    /// Below the standard tier; still progressive, never below one sample.
    Reduced,
    /// The standard adaptive detail tier.
    #[default]
    Standard,
    /// Maximum detail and sample budgets.
    High,
}

impl QualityTier {
    /// Every tier, cheapest first.
    pub const ALL: [Self; 4] = [Self::Minimal, Self::Reduced, Self::Standard, Self::High];

    /// Selects the largest tier allowed by scene size before frame-time
    /// adaptation.
    ///
    /// The bands bound expensive surface, temporal and upload work without
    /// touching coordinates. Callers may still force publication quality; this
    /// policy only constrains adaptive realtime rendering.
    #[must_use]
    pub const fn for_atom_count(atom_count: u64) -> Self {
        if atom_count <= 10_000 {
            Self::High
        } else if atom_count <= 100_000 {
            Self::Standard
        } else if atom_count <= 500_000 {
            Self::Reduced
        } else {
            Self::Minimal
        }
    }

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
    ///
    /// The two richer tiers share the finest spacing: it is the resolution at
    /// which a surface stops changing visibly, so a richer tier spends its
    /// budget on samples instead.
    #[must_use]
    pub const fn surface_grid_spacing(self) -> f32 {
        [0.75, 0.5, 0.25, 0.25][self.index()]
    }

    /// Maximum ribbon samples per trace interval for this tier.
    ///
    /// Fewer samples coarsen the spline between residues only; the residue
    /// positions the ribbon passes through are unchanged.
    #[must_use]
    pub const fn ribbon_steps(self) -> u8 {
        [3, 5, 8, 8][self.index()]
    }

    /// The scene-synchronization detail budgets this tier selects.
    #[must_use]
    pub(crate) const fn detail(self) -> crate::scene_gpu::detail::TierDetail {
        crate::scene_gpu::detail::TierDetail {
            surface_spacing: self.surface_grid_spacing(),
            ribbon_steps: self.ribbon_steps(),
            lod_enabled: !matches!(self, Self::High),
            surface_max_dimension: if matches!(self, Self::High) {
                u32::MAX
            } else {
                crate::scene_gpu::detail::INTERACTIVE_SURFACE_DIMENSION
            },
            surface_max_cells: if matches!(self, Self::High) {
                u32::MAX
            } else {
                crate::scene_gpu::detail::INTERACTIVE_SURFACE_DIMENSION.pow(3)
            },
        }
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

/// Rays per pixel for analytic occlusion and area-light visibility, per sample.
///
/// A converged output accumulates 64 samples, so the per-sample budget sets the
/// total (256 rays per pixel at the publication budget). Measured against a
/// 16-rays-per-sample reference of the same 64-sample output on a spacefill
/// scene, the RMS difference was 0.29, 0.47, 0.73 and 1.06 of 255 for 8, 4, 2 and
/// 1 rays: four is the largest saving that stays under the half-level
/// quantization noise of an 8-bit image. See `missing.md`.
pub(crate) const fn occlusion_rays(quality: bool, publication: bool) -> u8 {
    if !quality {
        0
    } else if publication {
        4
    } else {
        2
    }
}

/// Caller policy for the adaptive loop.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdaptiveQualityConfig {
    /// Frame rate the loop steers toward, in frames per second.
    pub target_fps: u16,
    /// Whether scene size and measured frame time may change the tier.
    pub enabled: bool,
    /// Initial adaptive tier, or the exact tier held when adaptation is disabled.
    pub initial_tier: QualityTier,
}

impl AdaptiveQualityConfig {
    /// An adapting loop steering toward `target_fps`.
    #[must_use]
    pub const fn interactive(target_fps: u16) -> Self {
        Self {
            target_fps,
            enabled: true,
            initial_tier: QualityTier::Reduced,
        }
    }

    /// Holds an exact tier, independent of scene size and frame duration.
    #[must_use]
    pub const fn fixed(target_fps: u16, tier: QualityTier) -> Self {
        Self {
            target_fps,
            enabled: false,
            initial_tier: tier,
        }
    }

    /// Maximum fixed detail for a caller-selected frame-rate target.
    #[must_use]
    pub const fn highest_fixed(target_fps: u16) -> Self {
        Self::fixed(target_fps, QualityTier::High)
    }

    /// Maximum fixed detail for deterministic publication output.
    #[must_use]
    pub const fn publication() -> Self {
        Self::highest_fixed(1)
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
    initial_tier: QualityTier,
    size_cap: QualityTier,
    overrun_frames: u32,
    headroom_frames: u32,
}

impl AdaptiveQuality {
    /// Builds a controller from the requested initial or fixed tier.
    ///
    /// Publication always selects maximum detail and never adapts. Leaving
    /// publication restores the caller-configured starting tier.
    #[must_use]
    pub const fn new(config: AdaptiveQualityConfig, publication: bool) -> Self {
        Self {
            requested: config.enabled,
            publication,
            target_fps: config.target_fps,
            target_ns: config.budget_ns(),
            ema_ns: 0,
            tier: if publication {
                QualityTier::High
            } else {
                config.initial_tier
            },
            initial_tier: config.initial_tier,
            size_cap: QualityTier::High,
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

    /// Whether this instant accumulates to a still image.
    ///
    /// The cinematic path and off-screen publication converge, so their
    /// sub-pixel coverage is already averaged; the realtime path does not, so
    /// its edges need explicit smoothing whatever tier it happens to hold.
    #[must_use]
    pub const fn converged(&self) -> bool {
        self.publication
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

    /// Applies the size-based realtime ceiling for the next frame.
    ///
    /// This is a cheap scene-level operation. Changing to a smaller band
    /// immediately drops the tier and clears its timing window; growing a
    /// scene's budget never causes a sudden expensive jump.
    pub fn set_atom_count(&mut self, atom_count: u64) {
        if !self.enabled() {
            return;
        }
        let cap = QualityTier::for_atom_count(atom_count);
        if cap == self.size_cap {
            return;
        }
        self.size_cap = cap;
        if self.tier > cap {
            self.tier = cap;
            self.reset_window();
        }
    }

    /// Marks the engine as running the deterministic publication path.
    ///
    /// Entering publication holds the tier constant from that frame on.
    pub fn set_publication(&mut self, publication: bool) {
        if self.publication != publication {
            self.publication = publication;
            self.tier = if publication {
                QualityTier::High
            } else {
                self.initial_tier
            };
            self.size_cap = QualityTier::High;
            self.reset_window();
        }
    }

    /// Feeds one frame's wall-clock duration and returns the tier the next
    /// frame presents at.
    ///
    /// The caller chooses the sample source per target: the native `render`
    /// call duration (CPU encoding/submission) or the browser
    /// submission-to-fence-completion elapsed time. Both are host-clock
    /// durations, neither is GPU execution time, and each frame feeds exactly
    /// one sample from exactly one source.
    pub fn observe(&mut self, frame_ns: u64) -> QualityTier {
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
            let next = match self.tier.richer() {
                Some(tier) if tier <= self.size_cap => Some(tier),
                _ => None,
            };
            self.step(next);
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

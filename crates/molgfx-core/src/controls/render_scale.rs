//! Adaptive resolution while the user interacts. Quality is only given up when the
//! GPU demonstrably cannot keep up: a repaint that arrives while the previous
//! frame is still running counts as a *skip*. A viewport that never skips stays
//! at full resolution, so a small structure is never softened.

/// The cost below which a frame is never heavy, milliseconds: about sixty a second.
const HEAVY_FLOOR_MS: f32 = 15.0;
/// How long a gesture must have gone on before a moderately dear picture is
/// simplified, milliseconds.
const GESTURE_MS: u32 = 150;
/// Render scales, from full resolution down.
const STEPS: [f32; 4] = [1.0, 0.75, 0.5, 0.35];
/// Observations considered when deciding to step down.
const WINDOW: u32 = 8;
/// Skips within the window that trigger a step down.
const SKIPS_TO_DROP: u32 = 3;
/// Consecutive clean interactive frames before stepping back up.
const CLEAN_TO_RISE: u32 = 40;

/// Chooses the render scale during interaction.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScaleController {
    step: usize,
    /// Bit i set: observation i ago was a skip (bit 0 is the latest).
    window: u32,
    clean_run: u32,
    locked_full: bool,
    pinned: Option<f32>,
    /// What a frame costs the GPU at full resolution, milliseconds, once measured.
    cost_ms: Option<f32>,
    /// What a frame may cost, milliseconds, to keep up with the display.
    budget_ms: Option<f32>,
    /// The measured cost is of a picture that is not the one being drawn (its
    /// surfaces are set aside), so it must not shrink the picture.
    plan_off: bool,
}

impl ScaleController {
    /// Starts at full resolution.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The scale to render at while interacting.
    #[must_use]
    pub fn scale(&self) -> f32 {
        if let Some(pinned) = self.pinned {
            pinned
        } else if self.locked_full {
            1.0
        } else {
            STEPS[self.step].min(self.planned())
        }
    }

    /// The largest step the measured cost allows within the budget, or full
    /// resolution while either is unknown. Pixel work shrinks with the square of
    /// the scale, so a frame that costs twice the budget fits at about 0.7.
    #[must_use]
    pub fn planned(&self) -> f32 {
        let (Some(cost), Some(budget)) = (self.cost_ms, self.budget_ms) else {
            return 1.0;
        };
        if self.plan_off {
            return 1.0;
        }
        if cost <= budget || cost <= 0.0 {
            return 1.0;
        }
        let wanted = (budget / cost).sqrt();
        match STEPS.iter().copied().find(|s| *s <= wanted) {
            Some(step) => step,
            None => STEPS[STEPS.len() - 1],
        }
    }

    /// The display budget when one was set, otherwise `fallback`.
    fn budget_or(&self, fallback: f32) -> f32 {
        let Some(budget) = self.budget_ms else {
            return fallback;
        };
        budget
    }

    /// Whether the GPU has fallen so far behind while turning that the resolution
    /// has been halved by the skips alone, whatever was measured at rest (which
    /// may not have happened yet: a surface takes seconds to settle).
    #[must_use]
    pub const fn is_lagging(&self) -> bool {
        self.step >= 2
    }

    /// Whether the surfaces should be swapped for stand-ins now: `zoom_only` is a
    /// scroll or pinch, not a drag, and `moving_ms` is how long the gesture has gone
    /// on. A frame that is far too dear (three times what the display allows) swaps
    /// at once for a drag and after a moment for a zoom; one that is dearer than
    /// the display allows by half swaps only when a drag has gone on a moment, so a
    /// tick of the wheel never makes the surface flash into dots and back. A GPU
    /// that has fallen behind in the frames it has really drawn always does.
    #[must_use]
    pub fn wants_aside(&self, zoom_only: bool, moving_ms: u32) -> bool {
        if self.is_lagging() {
            return true;
        }
        let Some(cost) = self.cost_ms else {
            return false;
        };
        let budget = self.budget_or(HEAVY_FLOOR_MS);
        let ratio = cost / budget.max(HEAVY_FLOOR_MS);
        if ratio > 3.0 {
            !zoom_only || moving_ms >= GESTURE_MS
        } else if ratio > 1.5 {
            !zoom_only && moving_ms >= GESTURE_MS
        } else {
            false
        }
    }

    /// Whether a picture of several large molecules is too dear for the display and
    /// has been turning for a moment: the molecules not being worked on may be
    /// drawn as points meanwhile. The display's budget decides (at least 8 ms: a
    /// frame that cheap is not worth simplifying).
    #[must_use]
    pub fn wants_crowd_aside(&self, moving_ms: u32) -> bool {
        let Some(cost) = self.cost_ms else {
            return false;
        };
        let budget = self.budget_or(16.7);
        moving_ms >= GESTURE_MS && cost > budget.max(8.0)
    }

    /// Turns the plan off while the picture drawn is not the one that was measured.
    pub fn set_plan_off(&mut self, off: bool) {
        self.plan_off = off;
    }

    /// Notes what a full-resolution frame costs the GPU, milliseconds.
    pub fn set_cost(&mut self, cost_ms: Option<f32>) {
        self.cost_ms = cost_ms.filter(|c| c.is_finite() && *c > 0.0);
    }

    /// Notes what a frame may cost to keep up with the display, milliseconds.
    pub fn set_budget(&mut self, budget_ms: Option<f32>) {
        self.budget_ms = budget_ms.filter(|b| b.is_finite() && *b > 0.0);
    }

    /// Holds one scale whatever happens (for measuring); `None` releases it.
    pub fn pin(&mut self, scale: Option<f32>) {
        self.pinned = scale.map(|s| s.clamp(0.1, 1.0));
    }

    /// Never reduce resolution (the user chose full quality).
    pub fn lock_full(&mut self, locked: bool) {
        self.locked_full = locked;
        if locked {
            self.step = 0;
            self.window = 0;
            self.clean_run = 0;
        }
    }

    /// Records one interactive repaint: `skipped` is true when the previous
    /// frame was still on the GPU.
    pub fn observe(&mut self, skipped: bool) {
        if self.locked_full || self.pinned.is_some() {
            return;
        }
        self.window = ((self.window << 1) | u32::from(skipped)) & ((1 << WINDOW) - 1);
        if skipped {
            self.clean_run = 0;
        } else {
            self.clean_run += 1;
        }
        if self.window.count_ones() >= SKIPS_TO_DROP && self.step + 1 < STEPS.len() {
            self.step += 1;
            self.window = 0;
            self.clean_run = 0;
        } else if self.clean_run >= CLEAN_TO_RISE && self.step > 0 {
            self.step -= 1;
            self.clean_run = 0;
        }
    }
}

#[cfg(test)]
#[path = "render_scale_tests.rs"]
mod tests;

//! Runtime engine settings and render-profile topology updates.

use super::{Engine, QualityTier, RenderMode, RenderProfile, ResolvedRenderPlan};
use crate::engine::graph_setup::realtime_nodes;
use crate::error::RenderError;
use crate::graph;
use molgfx_gpu::{Device, Surface as _, SurfaceConfig};

impl<D: Device> Engine<D> {
    /// Updates the frame size after a window resize; the surface and pool
    /// reconfigure lazily before the next frame.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.temporal.reset();
        if let Some(surface) = &mut self.surface {
            surface.configure(
                &self.device,
                &SurfaceConfig {
                    width: self.width,
                    height: self.height,
                    format: self.target_format,
                },
            );
        }
    }

    /// The opened device's capability report.
    #[must_use]
    pub fn capabilities(&self) -> &molgfx_gpu::Capabilities {
        self.device.capabilities()
    }

    /// Changes rendering strategy without rebuilding the scene or device.
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        if self.mode != mode {
            self.mode = mode;
            // The cinematic path is the deterministic publication path, so it
            // holds one tier.
            self.adaptive.set_publication(mode == RenderMode::Cinematic);
            self.temporal.reset();
        }
    }

    /// The adaptive quality tier the next frame presents at.
    #[must_use]
    pub const fn quality_tier(&self) -> QualityTier {
        self.adaptive.tier()
    }

    /// The tier every stage of the current frame reads.
    pub(crate) const fn tier(&self) -> QualityTier {
        self.adaptive.tier()
    }

    /// Publishes this frame's tier into the state the frame loop reads.
    ///
    /// Called at the top of every frame path, before any pass is built, so one
    /// frame never mixes two tiers. A tier move restarts accumulation, because
    /// history gathered under one sample budget is not a valid prefix of
    /// another.
    pub(crate) fn sync_quality_tier(&mut self) {
        if self.temporal.tier() != self.adaptive.tier() {
            self.temporal.set_tier(self.adaptive.tier());
            self.temporal.reset();
        }
    }

    /// Deterministic description of the adaptive quality loop: the tier it
    /// currently holds, the smoothed frame time that drives it, and the
    /// resolution that tier selects.
    #[must_use]
    pub fn explain(&self) -> String {
        format!(
            "quality tier: {:?}\nsmoothed frame time ns: {}\ntarget fps: {}\n{}",
            self.adaptive.tier(),
            self.adaptive.smoothed_ns(),
            self.adaptive.target_fps(),
            self.scene_gpu.specialization_report()
        )
    }

    /// Current rendering strategy.
    #[must_use]
    pub const fn render_mode(&self) -> RenderMode {
        self.mode
    }

    /// Resolves and applies a reusable presentation recipe. Parameter-only
    /// changes preserve scene and pipeline allocations; temporal history is
    /// reset so the previous recipe never bleeds into the new presentation.
    ///
    /// # Errors
    ///
    /// Returns a graph error if a topology-changing module cannot be
    /// scheduled.
    pub fn set_render_profile(&mut self, profile: RenderProfile) -> Result<(), RenderError> {
        if self.profile == profile {
            return Ok(());
        }
        let resolved_plan = profile.resolve();
        let old_topology = (
            self.resolved_plan.depth_of_field().is_some(),
            self.resolved_plan.bloom().is_some(),
            self.resolved_plan.motion_blur().is_some(),
        );
        let new_topology = (
            resolved_plan.depth_of_field().is_some(),
            resolved_plan.bloom().is_some(),
            resolved_plan.motion_blur().is_some(),
        );
        if old_topology != new_topology {
            let pass_nodes = realtime_nodes(new_topology.0, new_topology.1, new_topology.2);
            let order = graph::schedule(&pass_nodes)?;
            // Prepare new pipelines before replacing any live state so a
            // failed profile transition leaves the previous frame usable.
            let depth_of_field = if new_topology.0 && self.passes.depth_of_field.is_none() {
                Some(crate::passes::DepthOfFieldPass::new(
                    &self.device,
                    &self.scene_gpu.group0_layout,
                )?)
            } else {
                None
            };
            let bloom = if new_topology.1 && self.passes.bloom.is_none() {
                Some(crate::passes::BloomPass::new(
                    &self.device,
                    &self.scene_gpu.group0_layout,
                )?)
            } else {
                None
            };
            let motion_blur = if new_topology.2 && self.passes.motion_blur.is_none() {
                Some(crate::passes::MotionBlurPass::new(
                    &self.device,
                    &self.scene_gpu.group0_layout,
                )?)
            } else {
                None
            };
            self.passes.depth_of_field = if new_topology.0 {
                self.passes.depth_of_field.take().or(depth_of_field)
            } else {
                None
            };
            self.passes.bloom = if new_topology.1 {
                self.passes.bloom.take().or(bloom)
            } else {
                None
            };
            self.passes.motion_blur = if new_topology.2 {
                self.passes.motion_blur.take().or(motion_blur)
            } else {
                None
            };
            self.pass_nodes = pass_nodes;
            self.order = order;
            self.bindings = None;
            self.pool = None;
        }
        self.resolved_plan = resolved_plan;
        self.profile = profile;
        self.temporal.reset();
        Ok(())
    }

    /// Replaces the derived-resource budget.
    ///
    /// The new limit takes effect on the next frame, which releases whatever
    /// no longer fits; a released resource rebuilds on next use.
    pub fn set_derived_cache_budget(&mut self, budget: crate::DerivedCacheBudget) {
        self.derived_cache.set_budget(budget);
    }

    /// The caller-authored presentation recipe.
    #[must_use]
    pub const fn render_profile(&self) -> &RenderProfile {
        &self.profile
    }

    /// The sanitized plan consumed by the frame loop.
    #[must_use]
    pub const fn resolved_render_plan(&self) -> &ResolvedRenderPlan {
        &self.resolved_plan
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "settings_tests.rs"]
mod tests;

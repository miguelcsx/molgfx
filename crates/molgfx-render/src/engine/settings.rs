//! Runtime engine settings and render-profile topology updates.

use super::{Engine, RenderMode, RenderProfile, ResolvedRenderPlan};
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
            self.temporal.reset();
        }
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

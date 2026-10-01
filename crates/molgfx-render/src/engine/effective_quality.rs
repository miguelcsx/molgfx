//! Resolved quality and observed exposure completion, without inferred budgets.

use super::{Engine, QualityTier};
use molgfx_gpu::Device;

/// Physical output settings and the exposure work actually observed.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EffectiveQuality {
    /// Physical render-target width and height, in pixels.
    pub extent: [u32; 2],
    /// Tier selected before recording this output.
    pub tier: QualityTier,
    /// Samples required by the selected output policy.
    pub samples_required: u32,
    /// Samples submitted for this exposure.
    pub samples_submitted: u32,
    /// Samples whose completion was observed; absent before a fence/readback.
    pub samples_completed: Option<u32>,
    /// Requested sampled-surface spacing in ångström.
    pub surface_spacing_requested: f32,
    /// Minimum and maximum spacing of the live sampled fields; absent without fields.
    pub surface_spacing_effective: Option<[f32; 2]>,
    /// Selected maximum ribbon samples per trace interval.
    pub ribbon_steps_max: u8,
    /// Analytic occlusion and area-light rays per covered pixel per sample.
    pub occlusion_rays_per_sample: u8,
    /// Resolved diffuse/specular environment and direct-light settings.
    pub lighting: super::LightingEnvironment,
    /// Indirect transport iterations; zero when no GI transport is scheduled.
    pub illumination_bounces: u8,
    /// Largest active molecular cull LOD mode; zero preserves analytic geometry.
    pub lod_mode_max: u32,
    /// Explicit one-sample sequence mode, never a converged-output certificate.
    pub progressive: bool,
    /// Whether timing or scene size may adapt the selected tier.
    pub adaptive: bool,
    /// Whether all admitted source uploads were resident for this output.
    pub full_residency: bool,
}

impl serde::Serialize for EffectiveQuality {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        let mut state = serializer.serialize_struct("EffectiveQuality", 16)?;
        state.serialize_field("extent", &self.extent)?;
        state.serialize_field("tier", &self.tier)?;
        state.serialize_field("samples_required", &self.samples_required)?;
        state.serialize_field("samples_submitted", &self.samples_submitted)?;
        state.serialize_field("samples_completed", &self.samples_completed)?;
        state.serialize_field("surface_spacing_requested", &self.surface_spacing_requested)?;
        state.serialize_field("surface_spacing_effective", &self.surface_spacing_effective)?;
        state.serialize_field("ribbon_steps_max", &self.ribbon_steps_max)?;
        state.serialize_field("occlusion_rays_per_sample", &self.occlusion_rays_per_sample)?;
        state.serialize_field("lighting", &self.lighting)?;
        state.serialize_field("illumination_bounces", &self.illumination_bounces)?;
        state.serialize_field("lod_mode_max", &self.lod_mode_max)?;
        state.serialize_field("progressive", &self.progressive)?;
        state.serialize_field("adaptive", &self.adaptive)?;
        state.serialize_field("full_residency", &self.full_residency)?;
        state.serialize_field("complete", &self.complete())?;
        state.end()
    }
}

impl EffectiveQuality {
    pub(super) fn observe_completion(&mut self) {
        self.samples_completed = Some(self.samples_submitted);
    }

    /// Whether observed samples, residency and sampled detail satisfy the request.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.samples_required > 0
            && self
                .samples_completed
                .is_some_and(|count| count >= self.samples_required)
            && !self.progressive
            && self.lod_mode_max == 0
            && self.full_residency
            && self.surface_spacing_effective.is_none_or(|spacing| {
                spacing[1] <= self.surface_spacing_requested * (1.0 + f32::EPSILON)
            })
    }
}

impl<D: Device> Engine<D> {
    pub(super) fn effective_quality(
        &self,
        samples_required: u32,
        samples_submitted: u32,
    ) -> EffectiveQuality {
        let tier = self.tier();
        EffectiveQuality {
            extent: [self.width, self.height],
            tier,
            samples_required,
            samples_submitted,
            samples_completed: None,
            surface_spacing_requested: tier.surface_grid_spacing(),
            surface_spacing_effective: self.scene_gpu.surface_spacing_range(),
            ribbon_steps_max: tier.ribbon_steps(),
            occlusion_rays_per_sample: self.temporal.occlusion_rays(),
            lighting: self.resolved_plan.lighting(),
            illumination_bounces: 0,
            lod_mode_max: self.scene_gpu.lod_mode_max(),
            progressive: false,
            adaptive: self.adaptive.enabled(),
            full_residency: self.chunk_residency.metrics().uploads.active_tickets == 0,
        }
    }
}

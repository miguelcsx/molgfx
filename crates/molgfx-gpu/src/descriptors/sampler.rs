//! Sampler descriptors.

use super::pipeline::CompareFunction;

/// Texel filtering mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FilterMode {
    /// Nearest-texel sampling.
    Nearest,
    /// Bilinear sampling.
    #[default]
    Linear,
}

/// Everything needed to create a sampler.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SamplerDesc {
    /// Debug label.
    pub label: &'static str,
    /// Magnification and minification filter.
    pub filter: FilterMode,
    /// When set, the sampler is a comparison sampler (shadow maps).
    pub compare: Option<CompareFunction>,
}

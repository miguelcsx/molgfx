//! Higher-level semantic scene compositions.

pub(crate) mod difference;
pub(crate) mod ensemble;

pub use difference::{AtomCorrespondence, DifferenceScene, DifferenceStyle, DifferenceView};
pub use ensemble::{EnsembleScene, EnsembleStyle, EnsembleView, ProbabilityCloudView};

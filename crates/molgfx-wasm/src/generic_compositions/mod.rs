//! Batch-oriented browser adapters for meaning-agnostic visual compositions.

mod difference;
mod ensemble;
mod focus;
mod model;
mod values;

pub use difference::WebDifferenceComposition;
pub use ensemble::WebEnsembleComposition;
pub use model::WebGenericCompositionView;

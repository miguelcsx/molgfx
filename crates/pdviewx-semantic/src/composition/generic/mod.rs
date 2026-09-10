//! Meaning-agnostic compositions over row domains and caller attributes.

mod model;
mod scene;
mod validation;

pub use model::{
    CompositionError, DifferenceCompositionStyle, DifferenceLayer, EnsembleCompositionStyle,
    EnsembleLayer, FocusCompositionStyle, FocusLayer, GenericCompositionScene,
    GenericCompositionView,
};

#[cfg(test)]
mod tests;

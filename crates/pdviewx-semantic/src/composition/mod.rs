//! Higher-level semantic scene compositions.

pub(crate) mod generic;

pub use generic::{
    CompositionError, DifferenceCompositionStyle, DifferenceLayer, EnsembleCompositionStyle,
    EnsembleLayer, FocusCompositionStyle, FocusLayer, GenericCompositionScene,
    GenericCompositionView,
};

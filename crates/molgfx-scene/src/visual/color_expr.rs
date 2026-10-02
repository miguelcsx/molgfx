//! Color expressions of a visual program.

use super::{BoolExpr, Parameter, ScalarExpr};
use crate::Color;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Color expression.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum ColorExpr {
    /// Literal color.
    Constant(Color),
    /// Dynamically updateable uniform color.
    Parameter(Parameter<Color>),
    /// Conditional color in the typed visual program.
    Select {
        /// Branch predicate.
        condition: Arc<BoolExpr>,
        /// Color when the predicate is true.
        yes: Arc<Self>,
        /// Color when the predicate is false.
        no: Arc<Self>,
    },
    /// Named ramp over an explicit domain.
    Ramp {
        /// Scalar input.
        value: Arc<ScalarExpr>,
        /// Named palette name.
        palette: Box<str>,
        /// Explicit scalar domain.
        domain: [f32; 2],
        /// Color for unavailable input values.
        missing: Color,
    },
}

impl From<Color> for ColorExpr {
    fn from(value: Color) -> Self {
        Self::Constant(value)
    }
}

impl From<Parameter<Color>> for ColorExpr {
    fn from(value: Parameter<Color>) -> Self {
        Self::Parameter(value)
    }
}

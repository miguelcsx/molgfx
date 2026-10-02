//! Scalar expressions of a visual program.

use super::{BoolExpr, Parameter, VectorExpr};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Scalar expression evaluated from semantic entity data.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum ScalarExpr {
    /// Literal value.
    Constant(f32),
    /// Scene-bound atom property.
    Property(crate::ScalarProperty),
    /// Renderer-provided scalar input such as time or camera distance.
    Input(Box<str>),
    /// Dynamically updateable uniform.
    Parameter(Parameter<f32>),
    /// Addition.
    Add(Arc<Self>, Arc<Self>),
    /// Multiplication.
    Multiply(Arc<Self>, Arc<Self>),
    /// Clamps a value to a closed range.
    Clamp {
        /// Input expression.
        value: Arc<Self>,
        /// Lower bound.
        minimum: f32,
        /// Upper bound.
        maximum: f32,
    },
    /// Dot product of two vector expressions.
    VectorDot(Arc<VectorExpr>, Arc<VectorExpr>),
}

impl ScalarExpr {
    /// Scalar molecular property.
    #[must_use]
    pub fn property(property: crate::ScalarProperty) -> Self {
        Self::Property(property)
    }

    /// Named renderer intrinsic.
    #[must_use]
    pub fn input(name: impl Into<Box<str>>) -> Self {
        Self::Input(name.into())
    }

    /// Constant-folded and canonicalized expression.
    #[must_use]
    pub fn canonical(self) -> Self {
        match self {
            Self::Add(left, right) => fold_binary(left, right, true),
            Self::Multiply(left, right) => fold_binary(left, right, false),
            Self::Clamp {
                value,
                minimum,
                maximum,
            } => match value.as_ref() {
                Self::Constant(value) => Self::Constant(value.clamp(minimum, maximum)),
                _ => Self::Clamp {
                    value,
                    minimum,
                    maximum,
                },
            },
            Self::VectorDot(left, right) => Self::VectorDot(left, right),
            other => other,
        }
    }

    /// Clamps this expression to a finite closed interval.
    #[must_use]
    pub fn clamp(self, minimum: f32, maximum: f32) -> Self {
        Self::Clamp {
            value: Arc::new(self),
            minimum,
            maximum,
        }
        .canonical()
    }

    /// Less-than comparison.
    #[must_use]
    pub fn less(self, right: impl Into<Self>) -> BoolExpr {
        BoolExpr::Less(Arc::new(self), Arc::new(right.into())).canonical()
    }
}

fn fold_binary(left: Arc<ScalarExpr>, right: Arc<ScalarExpr>, add: bool) -> ScalarExpr {
    match (left.as_ref(), right.as_ref()) {
        (ScalarExpr::Constant(a), ScalarExpr::Constant(b)) => {
            ScalarExpr::Constant(if add { a + b } else { a * b })
        }
        _ if add => ScalarExpr::Add(left, right),
        _ => ScalarExpr::Multiply(left, right),
    }
}

impl std::ops::Add for ScalarExpr {
    type Output = Self;
    fn add(self, right: Self) -> Self {
        fold_binary(Arc::new(self), Arc::new(right), true)
    }
}

impl std::ops::Mul for ScalarExpr {
    type Output = Self;
    fn mul(self, right: Self) -> Self {
        fold_binary(Arc::new(self), Arc::new(right), false)
    }
}

impl From<f32> for ScalarExpr {
    fn from(value: f32) -> Self {
        Self::Constant(value)
    }
}

impl From<Parameter<f32>> for ScalarExpr {
    fn from(value: Parameter<f32>) -> Self {
        Self::Parameter(value)
    }
}

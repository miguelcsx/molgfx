//! Typed three-component vector expression values.

use crate::visual::{Parameter, ScalarExpr};
use serde::{Deserialize, Serialize};

/// Vector expression used by displacement and direction-aware styles.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum VectorExpr {
    /// Literal vector.
    Constant([f32; 3]),
    /// Named molecular vector property.
    Property(Box<str>),
    /// Dynamically updateable vector uniform.
    Parameter(Parameter<[f32; 3]>),
    /// Component-wise addition.
    Add(Box<Self>, Box<Self>),
    /// Scalar multiplication.
    Scale(Box<Self>, Box<ScalarExpr>),
    /// Defined normalization; a zero vector remains zero.
    Normalize(Box<Self>),
}

impl VectorExpr {
    /// Molecular vector property.
    #[must_use]
    pub fn property(name: impl Into<Box<str>>) -> Self {
        Self::Property(name.into())
    }

    /// Dot product with another vector.
    #[must_use]
    pub fn dot(self, right: Self) -> ScalarExpr {
        ScalarExpr::VectorDot(Box::new(self), Box::new(right))
    }

    /// Defined normalization; a zero vector remains zero.
    #[must_use]
    pub fn normalized(self) -> Self {
        Self::Normalize(Box::new(self))
    }
}

impl std::ops::Add for VectorExpr {
    type Output = Self;

    fn add(self, right: Self) -> Self {
        Self::Add(Box::new(self), Box::new(right))
    }
}

impl std::ops::Mul<ScalarExpr> for VectorExpr {
    type Output = Self;

    fn mul(self, right: ScalarExpr) -> Self {
        Self::Scale(Box::new(self), Box::new(right))
    }
}

impl From<[f32; 3]> for VectorExpr {
    fn from(value: [f32; 3]) -> Self {
        Self::Constant(value)
    }
}

impl From<Parameter<[f32; 3]>> for VectorExpr {
    fn from(value: Parameter<[f32; 3]>) -> Self {
        Self::Parameter(value)
    }
}

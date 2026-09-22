//! Typed immutable visual expressions compiled to specialized WGSL.

use crate::Color;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::marker::PhantomData;
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub(crate) mod compile;
mod intern;
pub(crate) mod native;
mod vector;

pub use compile::{CompiledVisual, VisualStage};
pub(crate) use compile::{PreparedVisual, ResolvedVisual};
pub use vector::VectorExpr;

/// Serialized value of one typed dynamic visual parameter.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ParameterValue {
    /// Scalar floating-point value.
    Scalar(f32),
    /// Linearizable semantic color value.
    Color(Color),
    /// Three-component vector value.
    Vector([f32; 3]),
}

mod parameter_type {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for crate::Color {}
    impl Sealed for [f32; 3] {}
}

/// Values supported by a typed [`Parameter`].
pub trait ParameterType: parameter_type::Sealed + Clone {
    #[doc(hidden)]
    fn into_parameter_value(self) -> ParameterValue;
}

impl ParameterType for f32 {
    fn into_parameter_value(self) -> ParameterValue {
        ParameterValue::Scalar(self)
    }
}

impl ParameterType for Color {
    fn into_parameter_value(self) -> ParameterValue {
        ParameterValue::Color(self)
    }
}

impl ParameterType for [f32; 3] {
    fn into_parameter_value(self) -> ParameterValue {
        ParameterValue::Vector(self)
    }
}

/// Stable typed identity for one dynamically updateable value.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Parameter<T> {
    name: Box<str>,
    default: T,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<T> Parameter<T> {
    /// Declares a named parameter with a default value.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, default: T) -> Self {
        Self {
            name: name.into(),
            default,
            marker: PhantomData,
        }
    }

    /// Stable source-level name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Default value used until scene state overrides it.
    #[must_use]
    pub const fn default_value(&self) -> &T {
        &self.default
    }
}

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

/// Boolean visual expression.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum BoolExpr {
    /// Literal value.
    Constant(bool),
    /// One GPU-resident interaction channel.
    State(Box<str>),
    /// Less-than comparison.
    Less(Arc<ScalarExpr>, Arc<ScalarExpr>),
    /// Boolean conjunction.
    And(Arc<Self>, Arc<Self>),
    /// Boolean disjunction.
    Or(Arc<Self>, Arc<Self>),
    /// Boolean negation.
    Not(Arc<Self>),
}

impl BoolExpr {
    /// GPU-resident semantic interaction state.
    #[must_use]
    pub fn state(name: impl Into<Box<str>>) -> Self {
        Self::State(name.into())
    }

    /// Constant-folded boolean expression.
    #[must_use]
    pub fn canonical(self) -> Self {
        match self {
            Self::Less(left, right) => {
                if let (ScalarExpr::Constant(left_value), ScalarExpr::Constant(right_value)) =
                    (left.as_ref(), right.as_ref())
                {
                    Self::Constant(left_value < right_value)
                } else {
                    Self::Less(left, right)
                }
            }
            Self::And(left, right) => fold_bool(left, right, true),
            Self::Or(left, right) => fold_bool(left, right, false),
            Self::Not(value) => match value.as_ref() {
                Self::Constant(value) => Self::Constant(!value),
                _ => Self::Not(value),
            },
            other => other,
        }
    }
}

fn fold_bool(left: Arc<BoolExpr>, right: Arc<BoolExpr>, and: bool) -> BoolExpr {
    match (left.as_ref(), right.as_ref(), and) {
        (BoolExpr::Constant(false), _, true) | (_, BoolExpr::Constant(false), true) => {
            BoolExpr::Constant(false)
        }
        (BoolExpr::Constant(true), _, true) => right.as_ref().clone(),
        (_, BoolExpr::Constant(true), true) | (_, BoolExpr::Constant(false), false) => {
            left.as_ref().clone()
        }
        (BoolExpr::Constant(true), _, false) | (_, BoolExpr::Constant(true), false) => {
            BoolExpr::Constant(true)
        }
        (BoolExpr::Constant(false), _, false) => right.as_ref().clone(),
        _ if and => BoolExpr::And(left, right),
        _ => BoolExpr::Or(left, right),
    }
}

impl std::ops::BitAnd for BoolExpr {
    type Output = Self;
    fn bitand(self, right: Self) -> Self {
        fold_bool(Arc::new(self), Arc::new(right), true)
    }
}

impl std::ops::BitOr for BoolExpr {
    type Output = Self;
    fn bitor(self, right: Self) -> Self {
        fold_bool(Arc::new(self), Arc::new(right), false)
    }
}

impl std::ops::Not for BoolExpr {
    type Output = Self;
    fn not(self) -> Self {
        Self::Not(Arc::new(self)).canonical()
    }
}

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
    /// Scientific ramp over an explicit domain.
    Ramp {
        /// Scalar input.
        value: Arc<ScalarExpr>,
        /// Scientific palette name.
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

/// Complete immutable style program.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VisualStyle {
    /// Entity color.
    pub color: ColorExpr,
    /// Entity opacity.
    pub opacity: ScalarExpr,
    /// Entity visibility predicate.
    pub visible: BoolExpr,
}

impl VisualStyle {
    /// Builds one immutable style from typed output expressions.
    #[must_use]
    pub fn new(
        color: impl Into<ColorExpr>,
        opacity: impl Into<ScalarExpr>,
        visible: BoolExpr,
    ) -> Self {
        Self {
            color: color.into(),
            opacity: opacity.into(),
            visible,
        }
    }

    /// Structural identity of this style's three expression graphs.
    ///
    /// Folded bottom-up over the graph, so recognizing an unchanged style costs
    /// a walk rather than a serialization and a digest. This is a cache key, not
    /// a persisted identity: [`Self::stable_hash`] remains the portable one.
    pub(crate) fn structural_key(&self) -> [intern::Key; 3] {
        let mut interner = intern::Interner::default();
        [
            interner.color(&self.color),
            interner.scalar(&self.opacity),
            interner.boolean(&self.visible),
        ]
    }

    /// Stable SHA-256 key for shader and pipeline caches.
    #[must_use]
    pub fn stable_hash(&self) -> String {
        let canonical = self.canonical();
        let bytes = match serde_json::to_vec(&canonical) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        format!("{:x}", Sha256::digest(bytes))
    }

    /// Deterministic explanation of inferred inputs and cache identity.
    #[must_use]
    pub fn explain(&self) -> String {
        match self.compile() {
            Ok(compiled) => compiled.explain(),
            Err(error) => format!("VisualStyle\ninvalid: {error}"),
        }
    }

    /// Emits the specialized WGSL expression body used by the shader cache.
    #[must_use]
    pub fn wgsl(&self) -> String {
        match self.compile() {
            Ok(compiled) => compiled.wgsl().to_owned(),
            Err(error) => format!("// invalid visual style: {error}\n"),
        }
    }

    fn canonical(&self) -> Self {
        Self {
            color: self.color.clone(),
            opacity: self.opacity.clone().canonical(),
            visible: self.visible.clone().canonical(),
        }
    }
}

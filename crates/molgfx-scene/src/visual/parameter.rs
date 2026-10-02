//! Dynamic visual parameters and their typed values.

use crate::Color;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

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

//! Typed immutable visual expressions compiled to specialized WGSL.

mod boolean;
mod color_expr;
pub(crate) mod compile;
mod intern;
pub(crate) mod native;
mod parameter;
mod scalar;
mod style;
#[cfg(test)]
mod tests;
mod vector;

pub use boolean::BoolExpr;
pub use color_expr::ColorExpr;
pub use compile::{CompiledVisual, VisualStage};
pub(crate) use compile::{PreparedVisual, ResolvedVisual};
pub use parameter::{Parameter, ParameterType, ParameterValue};
pub use scalar::ScalarExpr;
pub use style::VisualStyle;
pub use vector::VectorExpr;

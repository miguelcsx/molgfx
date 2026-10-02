//! Boolean expressions of a visual program.

use super::ScalarExpr;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

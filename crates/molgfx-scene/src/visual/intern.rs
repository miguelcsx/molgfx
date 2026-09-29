//! Bottom-up structural identity for expression graphs.
//!
//! Lowering deduplicates repeated subexpressions, which needs a key that is
//! equal exactly when two subtrees are. Serializing each subtree to produce
//! that key costs bytes proportional to the subtree, so keying every node that
//! way costs time quadratic in the depth of the graph — and it throws away the
//! sharing the `Arc` edges already provide.
//!
//! A structural hash folded upward costs one pass. Children reached through an
//! `Arc` are hashed once and memoized by address, so a graph that shares a
//! subtree pays for it once no matter how many parents point at it.

use super::{BoolExpr, ColorExpr, ScalarExpr, VectorExpr};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A subtree's structural identity.
pub(crate) type Key = u64;

/// How deeply expressions may nest.
///
/// Every pass over a graph — hashing, checking, emitting WGSL, lowering — walks
/// it recursively, so nesting depth is stack depth. The renderer's program is
/// bounded at far fewer instructions than this, so no graph that could actually
/// run comes close; the limit exists so that a generated graph that never could
/// run is reported as an error instead of ending the process.
pub(crate) const MAX_DEPTH: usize = 128;

/// Rejects a graph too deep to walk safely.
pub(crate) fn check_depth(depth: usize) -> Result<usize, crate::Error> {
    if depth > MAX_DEPTH {
        return Err(crate::Error::InvalidSpec(format!(
            "visual expression nests deeper than {MAX_DEPTH} levels"
        )));
    }
    Ok(depth)
}

#[derive(Default)]
pub(crate) struct Interner {
    scalars: HashMap<usize, Key>,
    vectors: HashMap<usize, Key>,
    booleans: HashMap<usize, Key>,
    colors: HashMap<usize, Key>,
}

/// Seeds each node's hash with its own tag, so that two differently-shaped
/// nodes with identical children cannot collide by construction.
fn begin(tag: u8) -> std::collections::hash_map::DefaultHasher {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tag.hash(&mut hasher);
    hasher
}

fn finish(hasher: &std::collections::hash_map::DefaultHasher) -> Key {
    hasher.finish()
}

/// Hashes floats by bit pattern: two expressions are structurally identical
/// only when their constants are, and bit equality is the strict reading.
fn scalar_bits(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

macro_rules! memoized {
    ($self:ident, $table:ident, $node:expr, $body:expr) => {{
        let address = Arc::as_ptr($node).cast::<()>() as usize;
        if let Some(key) = $self.$table.get(&address) {
            *key
        } else {
            let key = $body;
            let _ = $self.$table.insert(address, key);
            key
        }
    }};
}

impl Interner {
    pub(crate) fn scalar(&mut self, expression: &ScalarExpr) -> Key {
        match expression {
            ScalarExpr::Constant(value) => {
                let mut hasher = begin(0);
                scalar_bits(*value, &mut hasher);
                finish(&hasher)
            }
            ScalarExpr::Property(property) => {
                let mut hasher = begin(1);
                property.name().hash(&mut hasher);
                property.structure().hash(&mut hasher);
                finish(&hasher)
            }
            ScalarExpr::Input(name) => {
                let mut hasher = begin(2);
                name.hash(&mut hasher);
                finish(&hasher)
            }
            ScalarExpr::Parameter(parameter) => {
                let mut hasher = begin(3);
                parameter.name().hash(&mut hasher);
                finish(&hasher)
            }
            ScalarExpr::Add(left, right) => self.scalar_binary(4, left, right),
            ScalarExpr::Multiply(left, right) => self.scalar_binary(5, left, right),
            ScalarExpr::Clamp {
                value,
                minimum,
                maximum,
            } => {
                let value = self.scalar_child(value);
                let mut hasher = begin(6);
                value.hash(&mut hasher);
                scalar_bits(*minimum, &mut hasher);
                scalar_bits(*maximum, &mut hasher);
                finish(&hasher)
            }
            ScalarExpr::VectorDot(left, right) => {
                let left = self.vector_child(left);
                let right = self.vector_child(right);
                let mut hasher = begin(7);
                left.hash(&mut hasher);
                right.hash(&mut hasher);
                finish(&hasher)
            }
        }
    }

    pub(crate) fn vector(&mut self, expression: &VectorExpr) -> Key {
        match expression {
            VectorExpr::Constant(value) => {
                let mut hasher = begin(16);
                for component in value {
                    scalar_bits(*component, &mut hasher);
                }
                finish(&hasher)
            }
            VectorExpr::Property(name) => {
                let mut hasher = begin(17);
                name.hash(&mut hasher);
                finish(&hasher)
            }
            VectorExpr::Parameter(parameter) => {
                let mut hasher = begin(18);
                parameter.name().hash(&mut hasher);
                finish(&hasher)
            }
            VectorExpr::Add(left, right) => {
                let left = self.vector_child(left);
                let right = self.vector_child(right);
                let mut hasher = begin(19);
                left.hash(&mut hasher);
                right.hash(&mut hasher);
                finish(&hasher)
            }
            VectorExpr::Scale(value, scale) => {
                let value = self.vector_child(value);
                let scale = self.scalar_child(scale);
                let mut hasher = begin(20);
                value.hash(&mut hasher);
                scale.hash(&mut hasher);
                finish(&hasher)
            }
            VectorExpr::Normalize(value) => {
                let value = self.vector_child(value);
                let mut hasher = begin(21);
                value.hash(&mut hasher);
                finish(&hasher)
            }
        }
    }

    pub(crate) fn boolean(&mut self, expression: &BoolExpr) -> Key {
        match expression {
            BoolExpr::Constant(value) => {
                let mut hasher = begin(32);
                value.hash(&mut hasher);
                finish(&hasher)
            }
            BoolExpr::State(name) => {
                let mut hasher = begin(33);
                name.hash(&mut hasher);
                finish(&hasher)
            }
            BoolExpr::Less(left, right) => {
                let left = self.scalar_child(left);
                let right = self.scalar_child(right);
                let mut hasher = begin(34);
                left.hash(&mut hasher);
                right.hash(&mut hasher);
                finish(&hasher)
            }
            BoolExpr::And(left, right) => self.boolean_binary(35, left, right),
            BoolExpr::Or(left, right) => self.boolean_binary(36, left, right),
            BoolExpr::Not(value) => {
                let value = self.boolean_child(value);
                let mut hasher = begin(37);
                value.hash(&mut hasher);
                finish(&hasher)
            }
        }
    }

    pub(crate) fn color(&mut self, expression: &ColorExpr) -> Key {
        match expression {
            ColorExpr::Constant(color) => {
                let mut hasher = begin(48);
                color.hash(&mut hasher);
                finish(&hasher)
            }
            ColorExpr::Parameter(parameter) => {
                let mut hasher = begin(49);
                parameter.name().hash(&mut hasher);
                finish(&hasher)
            }
            ColorExpr::Select { condition, yes, no } => {
                let condition = self.boolean_child(condition);
                let yes = self.color_child(yes);
                let no = self.color_child(no);
                let mut hasher = begin(50);
                condition.hash(&mut hasher);
                yes.hash(&mut hasher);
                no.hash(&mut hasher);
                finish(&hasher)
            }
            ColorExpr::Ramp {
                value,
                palette,
                domain,
                missing,
            } => {
                let value = self.scalar_child(value);
                let mut hasher = begin(51);
                value.hash(&mut hasher);
                palette.hash(&mut hasher);
                scalar_bits(domain[0], &mut hasher);
                scalar_bits(domain[1], &mut hasher);
                missing.hash(&mut hasher);
                finish(&hasher)
            }
        }
    }

    fn scalar_binary(&mut self, tag: u8, left: &Arc<ScalarExpr>, right: &Arc<ScalarExpr>) -> Key {
        let left = self.scalar_child(left);
        let right = self.scalar_child(right);
        let mut hasher = begin(tag);
        left.hash(&mut hasher);
        right.hash(&mut hasher);
        finish(&hasher)
    }

    fn boolean_binary(&mut self, tag: u8, left: &Arc<BoolExpr>, right: &Arc<BoolExpr>) -> Key {
        let left = self.boolean_child(left);
        let right = self.boolean_child(right);
        let mut hasher = begin(tag);
        left.hash(&mut hasher);
        right.hash(&mut hasher);
        finish(&hasher)
    }

    fn scalar_child(&mut self, node: &Arc<ScalarExpr>) -> Key {
        memoized!(self, scalars, node, self.scalar(node))
    }

    fn vector_child(&mut self, node: &Arc<VectorExpr>) -> Key {
        memoized!(self, vectors, node, self.vector(node))
    }

    fn boolean_child(&mut self, node: &Arc<BoolExpr>) -> Key {
        memoized!(self, booleans, node, self.boolean(node))
    }

    fn color_child(&mut self, node: &Arc<ColorExpr>) -> Key {
        memoized!(self, colors, node, self.color(node))
    }
}

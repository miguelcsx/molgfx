//! The complete immutable style a representation carries.

use super::{BoolExpr, ColorExpr, ScalarExpr, intern};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

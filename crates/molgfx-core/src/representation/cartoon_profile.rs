//! Cross-section values shared by protein and nucleic-acid cartoon recipes.

/// Shape of a swept cartoon cross-section, without changing its guide curve.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartoonProfile {
    /// Smooth ellipse with the authored width and depth.
    #[default]
    Elliptical,
    /// Flat faces joined by semicircular ends.
    Rounded,
    /// Four planar faces with independent face normals.
    Square,
}

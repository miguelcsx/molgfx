//! Presentation of missing intervals in a polymer backbone.

/// Whether a real polymer gap has a visual connector.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapStyle {
    /// Leave the interval empty.
    #[default]
    Hidden,
    /// Draw a dashed tube between the surviving guide atoms.
    Dashed,
}

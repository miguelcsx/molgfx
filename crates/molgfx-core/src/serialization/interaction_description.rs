//! Serializable caller-computed interaction facts.

use super::{AnchorDescription, ObjectIdentity};
use serde::{Deserialize, Serialize};

/// One caller-computed interaction fact.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct InteractionDescription {
    /// Stable interaction row and generation.
    pub row: u32,
    /// Interaction handle generation.
    pub generation: u32,
    /// Owning structure identity.
    pub owner: ObjectIdentity,
    /// First interaction endpoint.
    pub start: AnchorDescription,
    /// Second interaction endpoint.
    pub end: AnchorDescription,
    /// Stable interaction class name.
    pub kind: String,
    /// Stable direction name.
    pub direction: String,
    /// Caller-computed distance in ångström.
    pub distance_angstrom: f32,
    /// Optional caller-computed angle in degrees.
    pub angle_degrees: Option<f32>,
    /// Optional occupancy and normalized strength.
    pub occupancy: Option<f32>,
    /// Optional normalized line-strength value.
    pub normalized_strength: Option<f32>,
    /// Optional deterministic presentation phase speed in pixels per frame.
    pub phase_speed_pixels_per_frame: f32,
    /// Caller-computed age for optional presentation persistence decay.
    pub persistence_age_frames: u32,
    /// Presentation half-life in frames; zero disables visual decay.
    pub persistence_half_life_frames: f32,
    /// Source computation or dataset identifier.
    pub provenance: String,
    /// Whether the edge participates in rendering.
    pub visible: bool,
}

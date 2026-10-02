//! The interaction channels a scene tracks.

use serde::{Deserialize, Serialize};

/// GPU-resident semantic interaction channel.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChannel {
    /// User selection.
    Selected,
    /// Current pointer hover.
    Hovered,
    /// Focus target.
    Focused,
    /// De-emphasized context.
    Muted,
    /// Explicitly hidden entities.
    Hidden,
    /// Namespaced application-defined state bit.
    Custom(Box<str>),
}

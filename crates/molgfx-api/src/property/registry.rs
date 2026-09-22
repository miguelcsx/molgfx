//! The single definition of what a visual program is allowed to read.
//!
//! Both the WGSL compiler and the renderer lowering resolve every named input
//! through this module. When they each kept their own list the two drifted: a
//! style naming an input only one of them knew would type-check during
//! authoring and then fail when it reached a representation. Resolving here
//! means a name is either understood by the whole pipeline or rejected at the
//! first place a caller can see it.

use crate::Error;

/// A scalar value the renderer resolves per entity before a program runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScalarInput {
    /// Seconds since the scene began animating.
    Time,
    /// Distance from the camera to the entity.
    CameraDistance,
    /// Index of the entity within its representation.
    EntityIndex,
    /// Opacity the representation resolved before shading.
    BaseOpacity,
    /// Material roughness resolved before shading.
    Roughness,
    /// Material specular level resolved before shading.
    Specular,
    /// Strength of the representation's shading model.
    MaterialStrength,
}

/// A vector value the renderer resolves per fragment before a program runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VectorInput {
    /// Position in the entity's local frame.
    LocalPosition,
    /// Position in world space.
    WorldPosition,
    /// Shading normal.
    Normal,
    /// Unit vector from the surface toward the camera.
    ViewDirection,
}

/// One semantic interaction channel readable as a per-entity bit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StateChannel {
    /// User selection.
    Selected,
    /// Current pointer hover.
    Hovered,
    /// Active focus target.
    Focused,
    /// De-emphasized context.
    Muted,
    /// Explicitly hidden entity.
    Hidden,
    /// A caller-named channel, resolved to a bounded bit index.
    Custom(u32),
}

/// Generates name parsing, listing and diagnostics for a closed input set.
macro_rules! named_inputs {
    ($type:ident, $unknown:literal, [$($name:literal => $variant:ident),* $(,)?]) => {
        impl $type {
            /// Resolves a name, or reports every name that would have worked.
            pub(crate) fn parse(name: &str) -> Result<Self, Error> {
                match name {
                    $($name => Ok(Self::$variant),)*
                    _ => Err(unknown($unknown, name, &[$($name),*])),
                }
            }

            /// The name this input is written as in a visual expression.
            pub(crate) const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }
    };
}

named_inputs!(ScalarInput, "scalar input", [
    "time" => Time,
    "camera_distance" => CameraDistance,
    "entity_index" => EntityIndex,
    "base_opacity" => BaseOpacity,
    "roughness" => Roughness,
    "specular" => Specular,
    "material_strength" => MaterialStrength,
]);

named_inputs!(VectorInput, "vector input", [
    "local_position" => LocalPosition,
    "world_position" => WorldPosition,
    "normal" => Normal,
    "view_direction" => ViewDirection,
]);

/// Built-in channel names, in bit order.
const BUILTIN_CHANNELS: [&str; 5] = ["selected", "hovered", "focused", "muted", "hidden"];

impl StateChannel {
    /// Resolves a channel name.
    ///
    /// A name outside the built-in set is a caller-defined channel. Those are
    /// numbered by the order the scene declares them, so resolution needs the
    /// scene's channel list rather than a fixed table.
    pub(crate) fn parse(name: &str, custom: &[Box<str>]) -> Result<Self, Error> {
        if let Some(index) = BUILTIN_CHANNELS.iter().position(|known| *known == name) {
            return Ok(match index {
                0 => Self::Selected,
                1 => Self::Hovered,
                2 => Self::Focused,
                3 => Self::Muted,
                _ => Self::Hidden,
            });
        }
        let Some(index) = custom.iter().position(|known| known.as_ref() == name) else {
            let mut known: Vec<&str> = BUILTIN_CHANNELS.to_vec();
            known.extend(custom.iter().map(std::convert::AsRef::as_ref));
            return Err(unknown("interaction channel", name, &known));
        };
        u32::try_from(index).map_or_else(
            |_| {
                Err(Error::InvalidSpec(
                    "too many interaction channels".to_owned(),
                ))
            },
            |index| Ok(Self::Custom(index)),
        )
    }

    /// The bit this channel occupies in one per-entity state word.
    pub(crate) fn mask(self) -> Result<u32, Error> {
        let state = match self {
            Self::Selected => molgfx_core::InteractionState::SELECTED,
            Self::Hovered => molgfx_core::InteractionState::HOVERED,
            Self::Focused => molgfx_core::InteractionState::FOCUSED,
            Self::Muted => molgfx_core::InteractionState::MUTED,
            Self::Hidden => molgfx_core::InteractionState::HIDDEN,
            Self::Custom(index) => match molgfx_core::InteractionState::custom(index) {
                Some(state) => state,
                None => {
                    return Err(Error::InvalidSpec(format!(
                        "custom interaction channel {index} exceeds the state word"
                    )));
                }
            },
        };
        Ok(state.bits())
    }
}

fn unknown(kind: &str, name: &str, known: &[&str]) -> Error {
    Error::InvalidSpec(format!(
        "unknown {kind} '{name}'; the renderer exposes {}",
        known.join(", ")
    ))
}

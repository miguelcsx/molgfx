//! The typed authoring commands.
//!
//! Text is one way to write these; an application or an agent constructs them
//! directly. Every value in a command is already validated — a name is a name,
//! an opacity is between zero and one, a form's controls belong to that form —
//! so a command that exists is one the session can plan.

use super::color::ColorValue;
use super::form::Form;
use super::name::Name;
use super::target::{QueryText, Target};
use super::value::Opacity;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A request to draw a target with a form.
///
/// `show` is idempotent: when the structure already has a layer drawing the
/// same target with the same form and geometry, that layer is made visible and
/// given any colour or opacity requested, and no second layer appears. Set
/// `duplicate` to draw an independent second layer anyway.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Show {
    /// The form and its geometric controls.
    pub form: Form,
    /// What to draw.
    pub target: Target,
    /// The layer's name; generated from the form when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Name>,
    /// The structure to draw from; required only when a scene has several.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Name>,
    /// Base colour of the layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorValue>,
    /// Opacity of the layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<Opacity>,
    /// Draw a second, independent layer even if an identical one exists.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub duplicate: bool,
}

impl Show {
    /// Shows `target` with `form`, every other field at its default.
    #[must_use]
    pub const fn new(form: Form, target: Target) -> Self {
        Self {
            form,
            target,
            layer: None,
            structure: None,
            color: None,
            opacity: None,
            duplicate: false,
        }
    }
}

/// One authoring operation.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Defines or redefines a named selection. Layers, colour rules and
    /// selections that refer to it follow the new definition.
    Select {
        /// The selection's name.
        name: Name,
        /// Its query, which may refer to other named selections.
        query: QueryText,
    },
    /// Removes a named selection that nothing refers to.
    Unselect {
        /// The selection to remove.
        name: Name,
    },
    /// Draws a target, idempotently.
    Show(Show),
    /// Makes a hidden layer visible again.
    Reveal {
        /// The layer.
        layer: Name,
    },
    /// Hides a layer without removing it.
    Hide {
        /// The layer.
        layer: Name,
    },
    /// Removes a layer.
    Remove {
        /// The layer.
        layer: Name,
    },
    /// Colours a target. A layer takes the colour as its base colour; a query
    /// adds a rule colouring those atoms in every layer of the structure.
    Color {
        /// The colouring.
        color: ColorValue,
        /// What to colour.
        target: Target,
        /// The structure a query target is evaluated in; required only when a
        /// scene has several.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        structure: Option<Name>,
    },
    /// Removes colour rules: every rule of the structure, or those whose
    /// target is exactly `target`.
    Uncolor {
        /// Only rules with this exact target.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<QueryText>,
        /// The structure whose rules to remove.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        structure: Option<Name>,
    },
    /// Sets a layer's opacity.
    Opacity {
        /// The opacity.
        value: Opacity,
        /// The layer.
        layer: Name,
    },
    /// Focuses a target: it is highlighted and framed, and its distant
    /// context is muted.
    Focus {
        /// What to focus.
        target: Target,
    },
    /// Clears the focus.
    Unfocus,
    /// Undoes the most recent edit.
    Undo,
    /// Redoes the most recently undone edit.
    Redo,
}

impl Command {
    /// The verb that writes this command.
    #[must_use]
    pub const fn verb(&self) -> &'static str {
        match self {
            Self::Select { .. } => "select",
            Self::Unselect { .. } => "unselect",
            Self::Show(_) | Self::Reveal { .. } => "show",
            Self::Hide { .. } => "hide",
            Self::Remove { .. } => "remove",
            Self::Color { .. } => "color",
            Self::Uncolor { .. } => "uncolor",
            Self::Opacity { .. } => "opacity",
            Self::Focus { .. } => "focus",
            Self::Unfocus => "unfocus",
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }

    /// Whether this command moves through history rather than editing.
    #[must_use]
    pub const fn is_history(&self) -> bool {
        matches!(self, Self::Undo | Self::Redo)
    }
}

/// The canonical text of a command, which parses back to the same command.
impl fmt::Display for Command {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let in_structure = |structure: &Option<Name>| {
            structure
                .as_ref()
                .map_or_else(String::new, |name| format!(" in {name}"))
        };
        match self {
            Self::Select { name, query } => write!(formatter, "select {name}, {query}"),
            Self::Unselect { name } => write!(formatter, "unselect {name}"),
            Self::Show(show) => {
                write!(formatter, "show {}", show.form.kind().name())?;
                super::form_text::write_options(formatter, &show.form)?;
                if let Some(color) = &show.color {
                    write!(formatter, " color={}", super::form_text::color_word(color))?;
                }
                if let Some(opacity) = show.opacity {
                    write!(formatter, " opacity={opacity}")?;
                }
                if show.duplicate {
                    formatter.write_str(" duplicate")?;
                }
                if let Some(layer) = &show.layer {
                    write!(formatter, " as {layer}")?;
                }
                write!(
                    formatter,
                    "{}, {}",
                    in_structure(&show.structure),
                    show.target
                )
            }
            Self::Reveal { layer } => write!(formatter, "show @{layer}"),
            Self::Hide { layer } => write!(formatter, "hide @{layer}"),
            Self::Remove { layer } => write!(formatter, "remove @{layer}"),
            Self::Color {
                color,
                target,
                structure,
            } => write!(
                formatter,
                "color {color}{}, {target}",
                in_structure(structure)
            ),
            Self::Uncolor { target, structure } => {
                write!(formatter, "uncolor{}", in_structure(structure))?;
                if let Some(target) = target {
                    write!(formatter, ", {target}")?;
                }
                Ok(())
            }
            Self::Opacity { value, layer } => write!(formatter, "opacity {value}, @{layer}"),
            Self::Focus { target } => write!(formatter, "focus {target}"),
            Self::Unfocus => formatter.write_str("unfocus"),
            Self::Undo => formatter.write_str("undo"),
            Self::Redo => formatter.write_str("redo"),
        }
    }
}

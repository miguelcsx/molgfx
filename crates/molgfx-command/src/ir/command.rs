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
use super::value::{Opacity, Positive};
use molgfx_scene::DomainSceneSnapshot as SceneSnapshot;
use molgfx_scene::{
    AssemblySpec, FitResult, InteractionSpec, MovieExportRequest, PlaneSpec, ValidationFinding,
    VolumeSpec,
};
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

/// What a measurement reads off its points.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasureKind {
    /// The distance between two points.
    Distance,
    /// The angle at the middle of three points.
    Angle,
    /// The signed torsion about the axis through the middle two of four points.
    Dihedral,
}

impl MeasureKind {
    /// Every kind, in the order a help listing shows them.
    pub const ALL: [Self; 3] = [Self::Distance, Self::Angle, Self::Dihedral];

    /// The verb that writes this measurement.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Distance => "distance",
            Self::Angle => "angle",
            Self::Dihedral => "dihedral",
        }
    }

    /// How many points define it.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Distance => 2,
            Self::Angle => 3,
            Self::Dihedral => 4,
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
    /// Replaces the current semantic selection, or clears it when absent.
    SetSelection {
        /// `MolFrame` query, with Session aliases resolved at execution time.
        query: Option<QueryText>,
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
    /// Adds a text label at the centroid of a query.
    Label {
        /// The label text.
        text: String,
        /// The atoms whose centroid anchors the label.
        target: QueryText,
        /// The structure the query is evaluated in; required only when a
        /// scene has several.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        structure: Option<Name>,
    },
    /// Adds a distance, angle or dihedral measured between query centroids.
    Measure {
        /// What is measured.
        kind: MeasureKind,
        /// One query per point, in order; as many as the kind's arity.
        points: Vec<QueryText>,
        /// The structure the queries are evaluated in.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        structure: Option<Name>,
    },
    /// Draws a structure with the size- and chemistry-appropriate default
    /// forms, the way opening it in a viewer would.
    ///
    /// Which forms those are is one policy, owned by the scene layer; the
    /// command only names the structure to apply it to, so a caller does not
    /// restate the policy per statement.
    Auto {
        /// The structure to draw; required only when a scene has several.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        structure: Option<Name>,
    },
    /// Draws the pocket-and-pose composition around a query: the subject at
    /// full detail, its interaction shell and orienting shell, a translucent
    /// pocket surface and local solvent, with everything farther demoted.
    ///
    /// The bands are one policy owned by the scene layer; the command carries
    /// only the subject and the two shell radii.
    Pocket {
        /// The subject the pocket is built around.
        target: QueryText,
        /// Interaction-shell radius in ångström; the scene default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        near: Option<Positive>,
        /// Orienting-shell radius in ångström; the scene default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mid: Option<Positive>,
        /// The structure to draw; required only when a scene has several.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        structure: Option<Name>,
    },
    /// Adds a caller-supplied explicit overlay interaction.
    ///
    /// The interaction is already resolved by the caller; command execution
    /// only validates and stages its scene patch.
    Interaction {
        /// The explicit interaction specification.
        interaction: InteractionSpec,
    },
    /// Retains crystallographic assembly and unit-cell metadata.
    Assembly {
        /// Crystallographic assembly metadata.
        assembly: Option<AssemblySpec>,
    },
    /// Adds a finite caller-authored planar guide.
    Plane {
        /// Immutable plane geometry and style.
        plane: PlaneSpec,
    },
    /// Declares a density volume whose grid values arrive through a runtime
    /// binding.
    ///
    /// The command carries the identity and sampling metadata only; the bulk
    /// grid is supplied to the renderer separately, exactly as a data source
    /// descriptor keeps the portable scene document small.
    Volume {
        /// Immutable volume metadata.
        volume: VolumeSpec,
    },
    /// Retains a validated native fitting result.
    Fitting {
        /// Validated fitting result.
        fitting: Option<FitResult>,
    },
    /// Retains caller-computed validation findings.
    Validation {
        /// Caller-computed validation findings.
        findings: Vec<ValidationFinding>,
    },
    /// Retains a deterministic native movie export request.
    MovieExport {
        /// Deterministic movie export request.
        request: Option<MovieExportRequest>,
    },
    /// Retains a validated portable snapshot for host restoration.
    Snapshot {
        /// Portable snapshot to restore.
        snapshot: Option<Box<SceneSnapshot>>,
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
            Self::SetSelection { .. } => "selection",
            Self::Show(_) | Self::Reveal { .. } => "show",
            Self::Hide { .. } => "hide",
            Self::Remove { .. } => "remove",
            Self::Color { .. } => "color",
            Self::Uncolor { .. } => "uncolor",
            Self::Opacity { .. } => "opacity",
            Self::Focus { .. } => "focus",
            Self::Label { .. } => "label",
            Self::Measure { kind, .. } => kind.name(),
            Self::Auto { .. } => "auto",
            Self::Pocket { .. } => "pocket",
            Self::Interaction { .. } => "interaction",
            Self::Assembly { .. } => "assembly",
            Self::Plane { .. } => "plane",
            Self::Volume { .. } => "volume",
            Self::Fitting { .. } => "fitting",
            Self::Validation { .. } => "validation",
            Self::MovieExport { .. } => "movie_export",
            Self::Snapshot { .. } => "snapshot",
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
            Self::SetSelection { query } => match query {
                Some(query) => write!(formatter, "selection {query}"),
                None => formatter.write_str("selection"),
            },
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
            Self::Label {
                text,
                target,
                structure,
            } => write!(
                formatter,
                "label \"{}\"{}, {target}",
                text.replace('\\', "\\\\").replace('"', "\\\""),
                in_structure(structure)
            ),
            Self::Measure {
                kind,
                points,
                structure,
            } => {
                write!(formatter, "{}{}", kind.name(), in_structure(structure))?;
                for point in points {
                    write!(formatter, ", {point}")?;
                }
                Ok(())
            }
            Self::Auto { structure } => {
                write!(formatter, "auto{}", in_structure(structure))
            }
            Self::Pocket {
                target,
                near,
                mid,
                structure,
            } => write_pocket(formatter, target, [*near, *mid], &in_structure(structure)),
            Self::Interaction { interaction } => write_json(formatter, "interaction", interaction),
            Self::Assembly { assembly } => write_json(formatter, "assembly", assembly),
            Self::Plane { plane } => write_json(formatter, "plane", plane),
            Self::Volume { volume } => write_json(formatter, "volume", volume),
            Self::Fitting { fitting } => write_json(formatter, "fitting", fitting),
            Self::Validation { findings } => write_json(formatter, "validation", findings),
            Self::MovieExport { request } => write_json(formatter, "movie_export", request),
            Self::Snapshot { snapshot } => write_json(formatter, "snapshot", snapshot),
            Self::Unfocus => formatter.write_str("unfocus"),
            Self::Undo => formatter.write_str("undo"),
            Self::Redo => formatter.write_str("redo"),
        }
    }
}

/// Writes `verb` followed by the JSON form of a caller-supplied value.
/// `pocket [near=N] [mid=M] [in S], TARGET`, with absent radii left out.
fn write_pocket(
    formatter: &mut fmt::Formatter<'_>,
    target: &QueryText,
    radii: [Option<Positive>; 2],
    structure: &impl fmt::Display,
) -> fmt::Result {
    write!(formatter, "pocket")?;
    for (key, radius) in ["near", "mid"].into_iter().zip(radii) {
        if let Some(radius) = radius {
            write!(formatter, " {key}={radius}")?;
        }
    }
    write!(formatter, "{structure}, {target}")
}

fn write_json<T: Serialize + ?Sized>(
    formatter: &mut fmt::Formatter<'_>,
    verb: &str,
    value: &T,
) -> fmt::Result {
    write!(
        formatter,
        "{verb} {}",
        serde_json::to_string(value).map_err(|_| fmt::Error)?
    )
}

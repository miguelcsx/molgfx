//! Representation forms and their typed, form-specific controls.
//!
//! One table below declares every form, the builder it lowers to, and each
//! control the form accepts together with the control's value type. The form
//! enum, the option parser, the lowering to a representation and the metadata
//! a user interface lists for completion are all generated from that table,
//! so a control exists in all four places or in none. A control belongs to one
//! form: `style` on a cartoon is a cartoon style, and `style` does not exist on
//! a spacefill at all.

use super::value::{Finite, Positive};
use molgfx_api::RepresentationSpec;
use molgfx_api::rep::{CartoonStyle, SurfaceKind, SurfaceStyle};
use serde::{Deserialize, Serialize};

/// How one control's value is written and checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionKind {
    /// A finite number greater than zero.
    Positive,
    /// Any finite number.
    Finite,
    /// One of a fixed set of words.
    Choice(&'static [&'static str]),
}

/// One control a form accepts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OptionInfo {
    /// The control's name, as written before `=`.
    pub name: &'static str,
    /// How its value is written.
    pub kind: OptionKind,
    /// One line describing the control.
    pub summary: &'static str,
}

/// A value type a form control can hold.
pub(crate) trait ControlValue: Sized + Copy {
    const KIND: OptionKind;
    type Native;

    fn parse(text: &str) -> Result<Self, String>;
    fn native(self) -> Self::Native;
    /// The value as written after `=`.
    fn word(self) -> String;
}

impl ControlValue for Positive {
    const KIND: OptionKind = OptionKind::Positive;
    type Native = f32;

    fn parse(text: &str) -> Result<Self, String> {
        let value = text
            .parse::<f32>()
            .map_err(|_| format!("'{text}' is not a number"))?;
        Self::new(value).map_err(str::to_owned)
    }

    fn native(self) -> f32 {
        self.get()
    }

    fn word(self) -> String {
        self.to_string()
    }
}

impl ControlValue for Finite {
    const KIND: OptionKind = OptionKind::Finite;
    type Native = f32;

    fn parse(text: &str) -> Result<Self, String> {
        let value = text
            .parse::<f32>()
            .map_err(|_| format!("'{text}' is not a number"))?;
        Self::new(value).map_err(str::to_owned)
    }

    fn native(self) -> f32 {
        self.get()
    }

    fn word(self) -> String {
        self.to_string()
    }
}

/// Declares a word-valued control type from its spellings.
macro_rules! choice {
    ($type:ty, [$($word:literal => $variant:ident),* $(,)?]) => {
        impl ControlValue for $type {
            const KIND: OptionKind = OptionKind::Choice(&[$($word),*]);
            type Native = $type;

            fn parse(text: &str) -> Result<Self, String> {
                match text {
                    $($word => Ok(<$type>::$variant),)*
                    other => Err(format!(
                        "'{other}' is not one of {}",
                        [$($word),*].join(", ")
                    )),
                }
            }

            fn native(self) -> $type {
                self
            }

            fn word(self) -> String {
                match self {
                    $(<$type>::$variant => $word.to_owned(),)*
                }
            }
        }
    };
}

choice!(CartoonStyle, [
    "ribbon" => Ribbon,
    "rocket" => Rocket,
    "nucleic_acid" => NucleicAcid,
    "glycan" => Glycan,
]);

choice!(SurfaceKind, [
    "van_der_waals" => VanDerWaals,
    "solvent_accessible" => SolventAccessible,
    "solvent_excluded" => SolventExcluded,
    "gaussian" => Gaussian,
]);

choice!(SurfaceStyle, [
    "solid" => Solid,
    "contour" => Contour,
    "dots" => Dots,
    "filled_contour" => FilledContour,
    "mesh" => Mesh,
]);

/// Where a layer draws and how it looks, apart from its form.
pub(crate) struct Look {
    pub(crate) structure: molgfx_api::StructureId,
    pub(crate) color: Option<molgfx_api::ColorSpec>,
    pub(crate) opacity: Option<f32>,
}

/// Why a control could not be set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OptionError {
    /// The form has no control of this name.
    Unknown {
        /// Controls the form does have.
        known: &'static [OptionInfo],
    },
    /// The value is not valid for the control.
    Value(String),
}

macro_rules! forms {
    (
        $(
            $(#[$doc:meta])*
            $variant:ident = $word:literal via $builder:ident {
                $( $option:ident : $type:ty => $summary:literal ),* $(,)?
            }
        )*
    ) => {
        /// A drawable form, without its controls.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum FormKind {
            $( $(#[$doc])* $variant, )*
        }

        impl FormKind {
            /// Every form, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            /// The form's name, as written after `show`.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $word,)*
                }
            }

            /// The form a name spells, if any.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($word => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// The controls this form accepts.
            #[must_use]
            pub const fn options(self) -> &'static [OptionInfo] {
                match self {
                    $(Self::$variant => &[
                        $(OptionInfo {
                            name: stringify!($option),
                            kind: <$type as ControlValue>::KIND,
                            summary: $summary,
                        }),*
                    ],)*
                }
            }
        }

        /// A form together with the controls set on it. An unset control
        /// takes the form's default.
        #[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
        #[serde(tag = "form", rename_all = "snake_case", deny_unknown_fields)]
        pub enum Form {
            $(
                $(#[$doc])*
                $variant {
                    $(
                        #[doc = $summary]
                        #[serde(default, skip_serializing_if = "Option::is_none")]
                        $option: Option<$type>,
                    )*
                },
            )*
        }

        impl Form {
            /// A form with every control at its default.
            #[must_use]
            pub const fn new(kind: FormKind) -> Self {
                match kind {
                    $(FormKind::$variant => Self::$variant { $($option: None),* },)*
                }
            }

            /// The form, without its controls.
            #[must_use]
            pub const fn kind(&self) -> FormKind {
                match self {
                    $(Self::$variant { .. } => FormKind::$variant,)*
                }
            }

            /// Sets one control from its written value.
            ///
            /// # Errors
            ///
            /// Returns [`OptionError::Unknown`] for a control this form does
            /// not have, and [`OptionError::Value`] for an invalid value.
            pub fn set_option(&mut self, name: &str, value: &str) -> Result<(), OptionError> {
                let known = self.kind().options();
                match self {
                    $(
                        Self::$variant { $($option),* } => {
                            $(
                                if name == stringify!($option) {
                                    *$option = Some(
                                        <$type as ControlValue>::parse(value)
                                            .map_err(OptionError::Value)?,
                                    );
                                    return Ok(());
                                }
                            )*
                            Err(OptionError::Unknown { known })
                        }
                    )*
                }
            }

            /// Every control set on this form, as `name=value` words.
            #[must_use]
            pub fn controls(&self) -> Vec<String> {
                let mut words = Vec::new();
                match *self {
                    $(
                        Self::$variant { $($option),* } => {
                            $(
                                if let Some(value) = $option {
                                    words.push(format!(
                                        "{}={}",
                                        stringify!($option),
                                        ControlValue::word(value)
                                    ));
                                }
                            )*
                        }
                    )*
                }
                words
            }

            /// The representation this form draws over `target`.
            pub(crate) fn specification(
                &self,
                target: molgfx_api::Selection,
                look: Look,
            ) -> RepresentationSpec {
                match *self {
                    $(
                        Self::$variant { $($option),* } => {
                            let mut builder = molgfx_api::rep::$builder(target)
                                .structure(look.structure);
                            $(
                                if let Some(value) = $option {
                                    builder = builder.$option(ControlValue::native(value));
                                }
                            )*
                            if let Some(color) = look.color {
                                builder = builder.color(color);
                            }
                            if let Some(opacity) = look.opacity {
                                builder = builder.opacity(opacity);
                            }
                            builder.into()
                        }
                    )*
                }
            }
        }
    };
}

forms! {
    /// Secondary-structure ribbon along the polymer backbone.
    Cartoon = "cartoon" via cartoon {
        width: Positive => "ribbon width in ångström",
        style: CartoonStyle => "ribbon recipe",
    }
    /// Small atom spheres joined by bond capsules.
    BallAndStick = "ball_and_stick" via ball_and_stick {
        radius: Positive => "atom sphere scale relative to the van der Waals radius",
        bond_radius: Positive => "bond radius in ångström",
    }
    /// Van der Waals spheres.
    Spacefill = "spacefill" via spacefill {
        radius: Positive => "sphere scale relative to the van der Waals radius",
    }
    /// Uniform-radius sticks.
    Licorice = "licorice" via licorice {
        radius: Positive => "atom junction scale",
        bond_radius: Positive => "stick radius in ångström",
    }
    /// Pixel-stable bond wires.
    Lines = "lines" via lines {
        width: Positive => "line width in pixels",
    }
    /// One point per atom.
    Points = "points" via points {
        size: Positive => "point diameter in pixels",
    }
    /// A molecular surface.
    Surface = "surface" via surface {
        kind: SurfaceKind => "which molecular boundary",
        style: SurfaceStyle => "how the boundary is drawn",
        probe_radius: Positive => "solvent probe radius in ångström",
        isolevel: Finite => "level-set threshold",
    }
    /// Nucleic-acid backbone ribbon.
    NucleicAcid = "nucleic_acid" via nucleic_acid {
        width: Positive => "ribbon width in ångström",
    }
    /// Nucleic-acid bases.
    Bases = "bases" via bases {
        radius: Positive => "atom sphere scale",
    }
    /// Base-pair slabs.
    BasePairs = "base_pairs" via base_pairs {
        radius: Positive => "atom sphere scale",
        bond_radius: Positive => "bond radius in ångström",
    }
    /// Glycan tree ribbon.
    Glycan = "glycan" via glycan {
        width: Positive => "ribbon width in ångström",
    }
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;

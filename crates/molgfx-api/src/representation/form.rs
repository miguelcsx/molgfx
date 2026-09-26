//! Canonical tagged representation wire values.

use crate::color::ColorSpec;
use crate::id::StructureId;
use crate::representation::{CartoonStyle, Selection, SurfaceKind, SurfaceStyle};
use crate::visual::{ParameterValue, VisualStyle};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// State shared by every molecular representation form.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub(crate) struct RepresentationCommonSpec {
    pub(crate) structure: Option<StructureId>,
    pub(crate) target: Selection,
    pub(crate) color: ColorSpec,
    pub(crate) opacity: f32,
    #[serde(default)]
    pub(crate) visual: Option<VisualStyle>,
    #[serde(default)]
    pub(crate) parameters: BTreeMap<Box<str>, ParameterValue>,
    pub(crate) visible: bool,
}

/// Form-specific representation data. Each variant carries only valid controls.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RepresentationFormSpec {
    Cartoon {
        width: f32,
        style: CartoonStyle,
    },
    BallAndStick {
        radius: f32,
        bond_radius: f32,
    },
    Spacefill {
        radius: f32,
    },
    Licorice {
        radius: f32,
        bond_radius: f32,
    },
    Lines {
        width: f32,
    },
    Points {
        size: f32,
    },
    Surface {
        surface: SurfaceKind,
        style: SurfaceStyle,
        probe_radius: f32,
        isolevel: f32,
    },
    NucleicAcid {
        width: f32,
    },
    Bases {
        radius: f32,
    },
    BasePairs {
        radius: f32,
        bond_radius: f32,
    },
    Glycan {
        width: f32,
    },
}

/// Immutable serializable representation specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RepresentationSpec {
    pub(crate) common: RepresentationCommonSpec,
    pub(crate) form: RepresentationFormSpec,
}

impl RepresentationSpec {
    pub(crate) fn new(target: Selection, form: RepresentationFormSpec) -> Self {
        Self {
            common: RepresentationCommonSpec {
                structure: None,
                target,
                color: ColorSpec::default(),
                opacity: 1.0,
                visual: None,
                parameters: BTreeMap::new(),
                visible: true,
            },
            form,
        }
    }

    /// Applies one immutable typed visual expression graph.
    #[must_use]
    pub fn visual(mut self, visual: VisualStyle) -> Self {
        self.common.visual = Some(visual);
        self
    }

    /// `MolFrame` query selecting the represented atoms.
    #[must_use]
    pub fn selection(&self) -> &str {
        self.common.target.source()
    }

    /// The query selecting the represented atoms.
    #[must_use]
    pub const fn target(&self) -> &Selection {
        &self.common.target
    }

    /// Base colour; selection-scoped appearance rules take precedence over it.
    #[must_use]
    pub const fn color(&self) -> &ColorSpec {
        &self.common.color
    }

    /// Stable identity of the drawn geometry: the form and its geometric
    /// controls, without target, colour, opacity, visibility or visual style.
    ///
    /// Two representations with equal geometry keys over the same structure
    /// and target draw the same shapes, which is what makes it the right key
    /// for deciding whether a requested representation already exists.
    #[must_use]
    pub fn geometry_key(&self) -> String {
        crate::spec::stable_json_hash(&self.form)
    }

    /// Form name, as the wire format spells it.
    #[must_use]
    pub const fn form_name(&self) -> &'static str {
        match &self.form {
            RepresentationFormSpec::Cartoon { .. } => "cartoon",
            RepresentationFormSpec::BallAndStick { .. } => "ball_and_stick",
            RepresentationFormSpec::Spacefill { .. } => "spacefill",
            RepresentationFormSpec::Licorice { .. } => "licorice",
            RepresentationFormSpec::Lines { .. } => "lines",
            RepresentationFormSpec::Points { .. } => "points",
            RepresentationFormSpec::Surface { .. } => "surface",
            RepresentationFormSpec::NucleicAcid { .. } => "nucleic_acid",
            RepresentationFormSpec::Bases { .. } => "bases",
            RepresentationFormSpec::BasePairs { .. } => "base_pairs",
            RepresentationFormSpec::Glycan { .. } => "glycan",
        }
    }

    /// Molecular asset this representation targets after scene insertion.
    #[must_use]
    pub const fn structure_id(&self) -> Option<StructureId> {
        self.common.structure
    }

    /// Effective opacity.
    #[must_use]
    pub const fn opacity(&self) -> f32 {
        self.common.opacity
    }

    /// Whether this representation participates in drawing.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.common.visible
    }

    /// Stable content hash used by semantic and shader caches.
    #[must_use]
    pub fn stable_hash(&self) -> String {
        crate::spec::stable_json_hash(self)
    }

    /// Deterministic representation explanation.
    #[must_use]
    pub fn explain(&self) -> String {
        format!(
            "Representation\nform: {:?}\nselection: {}\nhash: {}",
            self.form,
            self.selection(),
            self.stable_hash()
        )
    }

    pub(crate) fn validate(&self) -> Result<(), crate::Error> {
        self.validate_values()?;
        if let Some(visual) = &self.common.visual {
            let _ = visual.compile()?;
        }
        Ok(())
    }

    pub(crate) fn validate_values(&self) -> Result<(), crate::Error> {
        self.common.color.validate()?;
        if self.common.visual.is_none() && !self.common.parameters.is_empty() {
            return Err(crate::Error::InvalidSpec(
                "visual parameter values require a visual style".to_owned(),
            ));
        }
        if !self.common.opacity.is_finite() || !(0.0..=1.0).contains(&self.common.opacity) {
            return Err(crate::Error::InvalidSpec(
                "opacity must be finite and between zero and one".to_owned(),
            ));
        }
        match &self.form {
            RepresentationFormSpec::Cartoon { width, .. }
            | RepresentationFormSpec::Lines { width }
            | RepresentationFormSpec::NucleicAcid { width }
            | RepresentationFormSpec::Glycan { width } => positive("width", *width)?,
            RepresentationFormSpec::Points { size } => positive("point size", *size)?,
            RepresentationFormSpec::Spacefill { radius }
            | RepresentationFormSpec::Bases { radius } => positive("radius", *radius)?,
            RepresentationFormSpec::BallAndStick {
                radius,
                bond_radius,
            }
            | RepresentationFormSpec::Licorice {
                radius,
                bond_radius,
            }
            | RepresentationFormSpec::BasePairs {
                radius,
                bond_radius,
            } => {
                positive("radius", *radius)?;
                positive("bond radius", *bond_radius)?;
            }
            RepresentationFormSpec::Surface {
                probe_radius,
                isolevel,
                ..
            } => {
                positive("probe radius", *probe_radius)?;
                if !isolevel.is_finite() {
                    return Err(crate::Error::InvalidSpec(
                        "isolevel must be finite".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn positive(name: &str, value: f32) -> Result<(), crate::Error> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(crate::Error::InvalidSpec(format!(
        "{name} must be finite and positive"
    )))
}

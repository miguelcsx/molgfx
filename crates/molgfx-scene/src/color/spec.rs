//! The declarative colouring rule and its lowering to the engine.

use super::{
    AtomCategory, AtomMetric, Color, DerivedColumn, Legend, LegendStop, index_fraction, ramp_colors,
};
use molgfx_core::CategoryPalette;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Colour for values a ramp legend cannot place.
const MISSING: Color = Color::rgb(204, 204, 204);

/// Declarative coloring rule evaluated from shared molecular metadata.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColorSpec {
    /// Conventional element colors.
    #[default]
    Element,
    /// One constant color.
    Uniform {
        /// Constant color for every selected entity.
        color: Color,
    },
    /// Scalar property mapped through a named ramp.
    Property {
        /// Typed scene-bound scalar property.
        property: crate::ScalarProperty,
        /// Palette name such as `viridis`, `plasma` or `coolwarm`.
        ramp: Box<str>,
        /// Explicit numeric domain.
        domain: [f32; 2],
        /// Overlay units displayed by the legend.
        units: Option<Box<str>>,
        /// Color assigned to unavailable values.
        missing: Color,
    },
    /// A category the structure defines — chain, entity, molecule type,
    /// residue name, residue, secondary structure — through a palette.
    Category {
        /// What atoms are categorised by.
        by: AtomCategory,
        /// Palette name; the category's own default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        palette: Option<Box<str>>,
        /// Whether only carbon atoms take a colour and other elements keep
        /// their own.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        carbon_only: bool,
    },
    /// A value the structure defines for itself — occupancy, temperature
    /// factor, charge, hydropathy, chain position — through a ramp.
    Metric {
        /// Which value.
        metric: AtomMetric,
        /// Ramp name; the metric's own default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ramp: Option<Box<str>>,
        /// Numeric domain; the metric's own default when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<[f32; 2]>,
    },
}

impl ColorSpec {
    /// Colors by `category` on its default palette.
    #[must_use]
    pub const fn category(category: AtomCategory) -> Self {
        Self::Category {
            by: category,
            palette: None,
            carbon_only: false,
        }
    }

    /// Restricts a categorical scheme to carbon atoms.
    #[must_use]
    pub fn carbon_only(mut self) -> Self {
        if let Self::Category { carbon_only, .. } = &mut self {
            *carbon_only = true;
        }
        self
    }

    /// Selects the palette of a categorical scheme.
    #[must_use]
    pub fn with_palette(mut self, name: impl Into<Box<str>>) -> Self {
        if let Self::Category { palette, .. } = &mut self {
            *palette = Some(name.into());
        }
        self
    }

    /// Selects the ramp of a metric scheme.
    #[must_use]
    pub fn with_ramp(mut self, name: impl Into<Box<str>>) -> Self {
        if let Self::Metric { ramp, .. } = &mut self {
            *ramp = Some(name.into());
        }
        self
    }

    /// Selects the domain of a metric scheme.
    #[must_use]
    pub fn with_domain(mut self, range: [f32; 2]) -> Self {
        if let Self::Metric { domain, .. } = &mut self {
            *domain = Some(range);
        }
        self
    }

    /// The structure-derived column this colour reads, if any.
    #[must_use]
    pub const fn derived(&self) -> Option<DerivedColumn> {
        match self {
            Self::Category {
                by, carbon_only, ..
            } => Some(DerivedColumn::Category {
                by: *by,
                carbon_only: *carbon_only,
            }),
            Self::Metric { metric, .. } => Some(DerivedColumn::Metric(*metric)),
            _ => None,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), crate::Error> {
        match self {
            Self::Property {
                property,
                ramp,
                domain,
                ..
            } => {
                if property.name().trim().is_empty()
                    || ramp.trim().is_empty()
                    || !valid_domain(*domain)
                {
                    return Err(crate::Error::InvalidSpec(
                        "property colors require names and a finite increasing domain".to_owned(),
                    ));
                }
                let _ = ramp_colors(ramp)?;
            }
            Self::Category {
                by: category,
                palette,
                ..
            } => {
                let _ = category_palette(*category, palette.as_deref())?;
            }
            Self::Metric {
                metric,
                ramp,
                domain,
            } => {
                let _ = ramp_colors(metric_ramp(*metric, ramp.as_deref()))?;
                if domain.is_some_and(|range| !valid_domain(range)) {
                    return Err(crate::Error::InvalidSpec(
                        "metric colors require a finite increasing domain".to_owned(),
                    ));
                }
            }
            Self::Element | Self::Uniform { .. } => {}
        }
        Ok(())
    }

    /// Generates a compact deterministic legend for scalar colors.
    #[must_use]
    pub fn legend(&self) -> Option<Legend> {
        match self {
            Self::Property {
                property,
                ramp,
                domain,
                units,
                missing,
            } => Some(ramp_legend(
                property.name(),
                ramp,
                *domain,
                units.clone(),
                *missing,
            )),
            Self::Metric {
                metric,
                ramp,
                domain,
            } => Some(ramp_legend(
                metric.name(),
                metric_ramp(*metric, ramp.as_deref()),
                metric_domain(*metric, *domain),
                metric.units().map(Into::into),
                MISSING,
            )),
            _ => None,
        }
    }

    /// Lowers to the engine's scheme. A derived colour reads its column from
    /// `properties`, where the scene bound it before lowering.
    pub(crate) fn native(
        &self,
        properties: &BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
        structure: crate::StructureId,
    ) -> Result<molgfx_core::ColorScheme, crate::Error> {
        self.validate()?;
        Ok(match self {
            Self::Element => molgfx_core::ColorScheme::ByElement,
            Self::Uniform { color } => molgfx_core::ColorScheme::Uniform(color.native()),
            Self::Property {
                property,
                ramp,
                domain,
                missing,
                ..
            } => {
                if property.structure() != structure {
                    return Err(crate::Error::InvalidSpec(
                        "property color belongs to another structure".to_owned(),
                    ));
                }
                let handle = bound(properties, property.name())?;
                property_scheme(handle, ramp, *domain, *missing)?
            }
            Self::Category {
                by: category,
                palette,
                carbon_only,
            } => {
                let column = DerivedColumn::Category {
                    by: *category,
                    carbon_only: *carbon_only,
                };
                let handle = bound(properties, &column.property_name(structure))?;
                molgfx_core::ColorScheme::category(
                    handle,
                    category_palette(*category, palette.as_deref())?,
                )
            }
            Self::Metric {
                metric,
                ramp,
                domain,
            } => {
                let handle = bound(
                    properties,
                    &DerivedColumn::Metric(*metric).property_name(structure),
                )?;
                property_scheme(
                    handle,
                    metric_ramp(*metric, ramp.as_deref()),
                    metric_domain(*metric, *domain),
                    MISSING,
                )?
            }
        })
    }
}

/// The ramp a metric colour reads: the caller's, or the metric's own.
fn metric_ramp(metric: AtomMetric, chosen: Option<&str>) -> &str {
    let Some(name) = chosen else {
        return metric.ramp();
    };
    name
}

/// The domain a metric colour spans: the caller's, or the metric's own.
fn metric_domain(metric: AtomMetric, chosen: Option<[f32; 2]>) -> [f32; 2] {
    let Some(domain) = chosen else {
        return metric.domain();
    };
    domain
}

fn valid_domain(domain: [f32; 2]) -> bool {
    domain[0].is_finite() && domain[1].is_finite() && domain[0] < domain[1]
}

fn bound(
    properties: &BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    name: &str,
) -> Result<molgfx_core::AtomPropertyHandle, crate::Error> {
    properties
        .get(name)
        .copied()
        .ok_or_else(|| crate::Error::InvalidSpec(format!("property '{name}' is not bound")))
}

fn category_palette(
    category: AtomCategory,
    name: Option<&str>,
) -> Result<CategoryPalette, crate::Error> {
    let Some(name) = name else {
        return Ok(category.default_palette());
    };
    CategoryPalette::from_name(name).ok_or_else(|| {
        let known: Vec<_> = CategoryPalette::ALL
            .into_iter()
            .map(CategoryPalette::name)
            .collect();
        crate::Error::InvalidSpec(format!(
            "unknown palette '{name}'; known palettes are {}",
            known.join(", ")
        ))
    })
}

fn property_scheme(
    handle: molgfx_core::AtomPropertyHandle,
    ramp: &str,
    domain: [f32; 2],
    missing: Color,
) -> Result<molgfx_core::ColorScheme, crate::Error> {
    let colors: Vec<_> = ramp_colors(ramp)?.into_iter().map(Color::native).collect();
    Ok(molgfx_core::ColorScheme::ByProperty {
        property: handle,
        ramp: molgfx_core::ScalarRamp::evenly(domain, &colors)?,
        missing: missing.native(),
    })
}

fn ramp_legend(
    title: &str,
    ramp: &str,
    domain: [f32; 2],
    units: Option<Box<str>>,
    missing: Color,
) -> Legend {
    // A legend is descriptive and infallible; an unvalidated ramp name
    // describes the default rather than refusing to describe anything.
    let colors = match ramp_colors(ramp) {
        Ok(colors) => colors,
        Err(_) => super::ramps::default_colors(),
    };
    let last = colors.len().saturating_sub(1).max(1);
    let stops = colors
        .iter()
        .enumerate()
        .map(|(index, color)| LegendStop {
            value: domain[0] + (domain[1] - domain[0]) * index_fraction(index, last),
            color: *color,
        })
        .collect();
    Legend {
        title: title.into(),
        units,
        stops,
        missing,
    }
}

//! Typed immutable representation specifications.

use crate::color::ColorSpec;
use crate::spec::{RepresentationForm, RepresentationSpec};
use sha2::Digest as _;

/// A canonical `MolFrame` query carried by a representation specification.
#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Selection(Box<str>);

impl Selection {
    /// Canonical `MolFrame` query text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.0
    }

    /// Stable hash of `MolFrame`'s typed logical query plan.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn stable_hash(&self) -> Result<String, crate::Error> {
        let query = molframe::Query::compile(&self.0).map_err(|diagnostics| {
            crate::Error::InvalidSpec(format!("selection diagnostics: {diagnostics:?}"))
        })?;
        Ok(format!(
            "{:x}",
            sha2::Sha256::digest(format!("{:?}", query.logical_plan()))
        ))
    }

    /// Deterministic explanation of the canonical query identity.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn explain(&self) -> Result<String, crate::Error> {
        Ok(format!(
            "Selection\nsource: {}\nhash: {}",
            self.0,
            self.stable_hash()?
        ))
    }
}

impl From<&str> for Selection {
    fn from(source: &str) -> Self {
        Self(source.into())
    }
}

impl From<String> for Selection {
    fn from(source: String) -> Self {
        Self(source.into_boxed_str())
    }
}

impl From<molframe::Query> for Selection {
    fn from(query: molframe::Query) -> Self {
        Self(query.source().into())
    }
}

impl From<molframe::query::Builder> for Selection {
    fn from(builder: molframe::query::Builder) -> Self {
        Self(builder.source().into())
    }
}

/// Cartoon geometry recipe.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartoonStyle {
    /// Smooth secondary-structure ribbon.
    Ribbon,
    /// Discrete helix and strand solids.
    Rocket,
    /// Nucleic-acid backbone and base-aware ribbon.
    NucleicAcid,
    /// Glycosidic tree ribbon.
    Glycan,
}

/// Molecular-surface definition.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// Van der Waals boundary.
    VanDerWaals,
    /// Solvent-accessible boundary.
    SolventAccessible,
    /// Solvent-excluded boundary.
    SolventExcluded,
    /// Gaussian density boundary.
    Gaussian,
}

/// Molecular-surface presentation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceStyle {
    /// Filled boundary.
    Solid,
    /// Contour lines.
    Contour,
    /// Dot lattice.
    Dots,
    /// Filled boundary with contours.
    FilledContour,
    /// Wire lattice.
    Mesh,
}

mod private {
    pub trait Sealed {}
}

/// A value accepted by [`crate::Scene::add`].
pub trait SceneItem: private::Sealed {
    #[doc(hidden)]
    fn into_spec(self) -> RepresentationSpec;
}

macro_rules! common_representation {
    ($name:ident) => {
        impl $name {
            /// Applies a shared molecular color rule.
            #[must_use]
            pub fn color(mut self, color: ColorSpec) -> Self {
                self.0.color = color;
                self
            }

            /// Sets opacity in the closed interval zero to one.
            #[must_use]
            pub fn opacity(mut self, opacity: f32) -> Self {
                self.0.opacity = opacity;
                self
            }

            /// Applies one immutable typed visual expression graph.
            #[must_use]
            pub fn visual(mut self, visual: crate::VisualStyle) -> Self {
                self.0.visual = Some(visual);
                self
            }

            /// Targets a specific molecular asset in a multi-structure scene.
            #[must_use]
            pub fn structure(mut self, structure: crate::StructureId) -> Self {
                self.0.structure = Some(structure);
                self
            }

            /// Stable canonical identity used by caches and diagnostics.
            #[must_use]
            pub fn stable_hash(&self) -> String {
                self.0.stable_hash()
            }

            /// Deterministic description of the semantic representation.
            #[must_use]
            pub fn explain(&self) -> String {
                self.0.explain()
            }
        }

        impl private::Sealed for $name {}

        impl SceneItem for $name {
            fn into_spec(self) -> RepresentationSpec {
                self.0
            }
        }
    };
}

/// Typed cartoon specification.
#[derive(Clone, PartialEq, Debug)]
pub struct Cartoon(RepresentationSpec);

impl Cartoon {
    /// Sets ribbon width in ångström.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.0.width = Some(width);
        self
    }

    /// Selects a cartoon recipe without changing its semantic target.
    #[must_use]
    pub fn style(mut self, style: CartoonStyle) -> Self {
        self.0.cartoon_style = Some(style);
        self
    }
}

common_representation!(Cartoon);

/// Typed atom/bond specification.
#[derive(Clone, PartialEq, Debug)]
pub struct AtomRepresentation(RepresentationSpec);

impl AtomRepresentation {
    /// Scales atom radii.
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.0.radius = Some(radius);
        self
    }

    /// Sets bond radius in ångström.
    #[must_use]
    pub fn bond_radius(mut self, radius: f32) -> Self {
        self.0.bond_radius = Some(radius);
        self
    }
}

common_representation!(AtomRepresentation);

/// Typed point specification.
#[derive(Clone, PartialEq, Debug)]
pub struct PointRepresentation(RepresentationSpec);

impl PointRepresentation {
    /// Sets point diameter in physical pixels.
    #[must_use]
    pub fn size(mut self, pixels: f32) -> Self {
        self.0.width = Some(pixels);
        self
    }
}

common_representation!(PointRepresentation);

/// Typed molecular-surface specification.
#[derive(Clone, PartialEq, Debug)]
pub struct Surface(RepresentationSpec);

impl Surface {
    /// Selects the physical boundary definition.
    #[must_use]
    pub fn kind(mut self, kind: SurfaceKind) -> Self {
        self.0.surface_kind = Some(kind);
        self
    }

    /// Selects surface presentation.
    #[must_use]
    pub fn style(mut self, style: SurfaceStyle) -> Self {
        self.0.surface_style = Some(style);
        self
    }

    /// Sets solvent probe radius in ångström.
    #[must_use]
    pub fn probe_radius(mut self, radius: f32) -> Self {
        self.0.probe_radius = Some(radius);
        self
    }

    /// Sets the level for Gaussian surfaces.
    #[must_use]
    pub fn isolevel(mut self, level: f32) -> Self {
        self.0.isolevel = Some(level);
        self
    }
}

common_representation!(Surface);

fn specification(target: impl Into<Selection>, form: RepresentationForm) -> RepresentationSpec {
    RepresentationSpec::new(target.into(), form)
}

/// Protein or polymer cartoon.
#[must_use]
pub fn cartoon(target: impl Into<Selection>) -> Cartoon {
    Cartoon(specification(target, RepresentationForm::Cartoon))
}

/// Small atom spheres and bond capsules.
#[must_use]
pub fn ball_and_stick(target: impl Into<Selection>) -> AtomRepresentation {
    AtomRepresentation(specification(target, RepresentationForm::BallAndStick))
}

/// Van der Waals atom spheres.
#[must_use]
pub fn spacefill(target: impl Into<Selection>) -> AtomRepresentation {
    AtomRepresentation(specification(target, RepresentationForm::Spacefill))
}

/// Uniform-radius atom junctions and bonds.
#[must_use]
pub fn licorice(target: impl Into<Selection>) -> AtomRepresentation {
    AtomRepresentation(specification(target, RepresentationForm::Licorice))
}

/// Pixel-stable bond lines.
#[must_use]
pub fn lines(target: impl Into<Selection>) -> AtomRepresentation {
    AtomRepresentation(specification(target, RepresentationForm::Lines))
}

/// Analytic points.
#[must_use]
pub fn points(target: impl Into<Selection>) -> PointRepresentation {
    PointRepresentation(specification(target, RepresentationForm::Points))
}

/// Molecular implicit surface.
#[must_use]
pub fn surface(target: impl Into<Selection>) -> Surface {
    Surface(specification(target, RepresentationForm::Surface))
}

/// Nucleic-acid-aware cartoon recipe.
#[must_use]
pub fn nucleic_acid(target: impl Into<Selection>) -> Cartoon {
    let mut specification = specification(target, RepresentationForm::NucleicAcid);
    specification.cartoon_style = Some(CartoonStyle::NucleicAcid);
    Cartoon(specification)
}

/// Explicit nucleic bases.
#[must_use]
pub fn bases(target: impl Into<Selection>) -> AtomRepresentation {
    AtomRepresentation(specification(target, RepresentationForm::Bases))
}

/// Paired nucleic bases with atomically selected pair context.
#[must_use]
pub fn base_pairs(target: impl Into<Selection>) -> AtomRepresentation {
    AtomRepresentation(specification(target, RepresentationForm::BasePairs))
}

/// SNFG/glycosidic-tree representation.
#[must_use]
pub fn glycan(target: impl Into<Selection>) -> Cartoon {
    let mut specification = specification(target, RepresentationForm::Glycan);
    specification.cartoon_style = Some(CartoonStyle::Glycan);
    Cartoon(specification)
}

//! Typed representation builders.
//!
//! Each builder owns exactly the controls its form defines, so a setter can
//! never address a field the form does not have. The form value is assembled
//! once, when the builder is converted into a specification — there is no
//! partially-built specification carrying fields from a different form.

use super::form::{RepresentationFormSpec, RepresentationSpec};
use super::item::{SceneItem, private};
use super::{CartoonStyle, Selection, SurfaceKind, SurfaceStyle};
use crate::color::ColorSpec;

/// Controls shared by every representation, independent of its form.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Common {
    target: Selection,
    color: ColorSpec,
    opacity: f32,
    visual: Option<crate::VisualStyle>,
    structure: Option<crate::StructureId>,
}

impl Common {
    fn new(target: impl Into<Selection>) -> Self {
        Self {
            target: target.into(),
            color: ColorSpec::default(),
            opacity: 1.0,
            visual: None,
            structure: None,
        }
    }

    fn specification(self, form: RepresentationFormSpec) -> RepresentationSpec {
        let mut spec = RepresentationSpec::new(self.target, form);
        spec.common.color = self.color;
        spec.common.opacity = self.opacity;
        spec.common.visual = self.visual;
        spec.common.structure = self.structure;
        spec
    }
}

/// Declares one typed builder over the controls of a single form.
///
/// `setter => field` keeps the two vocabularies independent: the builder names
/// its control the way a caller reads it, and the wire schema keeps its own
/// field name.
macro_rules! representation {
    (
        $(#[$doc:meta])*
        $name:ident => $variant:ident {
            $(
                $(#[$field_doc:meta])*
                $setter:ident => $field:ident : $type:ty = $default:expr
            ),* $(,)?
        }
    ) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Debug)]
        pub struct $name {
            common: Common,
            $($setter: $type,)*
        }

        impl $name {
            fn new(target: impl Into<Selection>) -> Self {
                Self {
                    common: Common::new(target),
                    $($setter: $default,)*
                }
            }

            $(
                $(#[$field_doc])*
                #[must_use]
                pub fn $setter(mut self, $setter: $type) -> Self {
                    self.$setter = $setter;
                    self
                }
            )*

            /// Applies a shared molecular color rule.
            #[must_use]
            pub fn color(mut self, color: ColorSpec) -> Self {
                self.common.color = color;
                self
            }

            /// Sets opacity in the closed interval zero to one.
            #[must_use]
            pub fn opacity(mut self, opacity: f32) -> Self {
                self.common.opacity = opacity;
                self
            }

            /// Applies one immutable typed visual expression graph.
            #[must_use]
            pub fn visual(mut self, visual: crate::VisualStyle) -> Self {
                self.common.visual = Some(visual);
                self
            }

            /// Targets a specific molecular asset in a multi-structure scene.
            #[must_use]
            pub fn structure(mut self, structure: crate::StructureId) -> Self {
                self.common.structure = Some(structure);
                self
            }

            /// Stable canonical identity used by caches and diagnostics.
            #[must_use]
            pub fn stable_hash(&self) -> String {
                RepresentationSpec::from(self.clone()).stable_hash()
            }

            /// Deterministic description of the semantic representation.
            #[must_use]
            pub fn explain(&self) -> String {
                RepresentationSpec::from(self.clone()).explain()
            }
        }

        impl From<$name> for RepresentationSpec {
            fn from(value: $name) -> Self {
                let $name { common, $($setter,)* } = value;
                common.specification(RepresentationFormSpec::$variant { $($field: $setter,)* })
            }
        }

        impl private::Sealed for $name {}

        impl SceneItem for $name {
            type Id = crate::RepresentationId;

            fn add_to(self, scene: &mut crate::Scene) -> Result<Self::Id, crate::Error> {
                scene.insert_representation(RepresentationSpec::from(self))
            }
        }
    };
}

representation! {
    /// Typed cartoon specification.
    Cartoon => Cartoon {
        /// Sets ribbon width in ångström.
        width => width: f32 = 1.2,
        /// Selects a cartoon recipe without changing its semantic target.
        style => style: CartoonStyle = CartoonStyle::Ribbon,
    }
}

representation! {
    /// Typed ball-and-stick specification.
    BallAndStick => BallAndStick {
        /// Scales atom radii.
        radius => radius: f32 = 0.25,
        /// Sets bond radius in ångström.
        bond_radius => bond_radius: f32 = 0.18,
    }
}

representation! {
    /// Typed space-filling atom representation.
    Spacefill => Spacefill {
        /// Scales atom radii.
        radius => radius: f32 = 1.0,
    }
}

representation! {
    /// Typed licorice representation.
    Licorice => Licorice {
        /// Scales atom radii.
        radius => radius: f32 = 1.0,
        /// Sets bond radius in ångström.
        bond_radius => bond_radius: f32 = 0.18,
    }
}

representation! {
    /// Typed line specification.
    Lines => Lines {
        /// Sets line diameter in physical pixels.
        width => width: f32 = 1.5,
    }
}

representation! {
    /// Typed point specification.
    PointRepresentation => Points {
        /// Sets point diameter in physical pixels.
        size => size: f32 = 3.0,
    }
}

representation! {
    /// Typed molecular-surface specification.
    Surface => Surface {
        /// Selects the physical boundary definition.
        kind => surface: SurfaceKind = SurfaceKind::SolventExcluded,
        /// Selects surface presentation.
        style => style: SurfaceStyle = SurfaceStyle::Solid,
        /// Sets solvent probe radius in ångström.
        probe_radius => probe_radius: f32 = 1.4,
        /// Sets the level for Gaussian surfaces.
        isolevel => isolevel: f32 = 0.0,
    }
}

representation! {
    /// Typed nucleic-acid backbone and base representation.
    NucleicAcid => NucleicAcid {
        /// Sets ribbon width in ångström.
        width => width: f32 = 1.2,
    }
}

representation! {
    /// Typed nucleic-base representation.
    Bases => Bases {
        /// Scales atom radii.
        radius => radius: f32 = 0.25,
    }
}

representation! {
    /// Typed nucleic base-pair representation.
    BasePairs => BasePairs {
        /// Scales atom radii.
        radius => radius: f32 = 0.25,
        /// Sets bond radius in ångström.
        bond_radius => bond_radius: f32 = 0.18,
    }
}

representation! {
    /// Typed SNFG glycan representation.
    Glycan => Glycan {
        /// Sets ribbon width in ångström.
        width => width: f32 = 1.2,
    }
}

/// Protein or polymer cartoon.
#[must_use]
pub fn cartoon(target: impl Into<Selection>) -> Cartoon {
    Cartoon::new(target)
}

/// Small atom spheres and bond capsules.
#[must_use]
pub fn ball_and_stick(target: impl Into<Selection>) -> BallAndStick {
    BallAndStick::new(target)
}

/// Van der Waals atom spheres.
#[must_use]
pub fn spacefill(target: impl Into<Selection>) -> Spacefill {
    Spacefill::new(target)
}

/// Uniform-radius atom junctions and bonds.
#[must_use]
pub fn licorice(target: impl Into<Selection>) -> Licorice {
    Licorice::new(target)
}

/// Pixel-stable bond lines.
#[must_use]
pub fn lines(target: impl Into<Selection>) -> Lines {
    Lines::new(target)
}

/// Analytic points.
#[must_use]
pub fn points(target: impl Into<Selection>) -> PointRepresentation {
    PointRepresentation::new(target)
}

/// Molecular implicit surface.
#[must_use]
pub fn surface(target: impl Into<Selection>) -> Surface {
    Surface::new(target)
}

/// Nucleic-acid-aware cartoon recipe.
#[must_use]
pub fn nucleic_acid(target: impl Into<Selection>) -> NucleicAcid {
    NucleicAcid::new(target)
}

/// Explicit nucleic bases.
#[must_use]
pub fn bases(target: impl Into<Selection>) -> Bases {
    Bases::new(target)
}

/// Paired nucleic bases with atomically selected pair context.
#[must_use]
pub fn base_pairs(target: impl Into<Selection>) -> BasePairs {
    BasePairs::new(target)
}

/// SNFG/glycosidic-tree representation.
#[must_use]
pub fn glycan(target: impl Into<Selection>) -> Glycan {
    Glycan::new(target)
}

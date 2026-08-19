//! Python adapters for declarative representation descriptors.

use super::PyMaterial;
use crate::math::PyRgba8;
use crate::values::{
    PyClipSet, PyColorScheme, PySegmentationStyle, PySurfaceKind, PySurfaceScalarOverlay,
    PySurfaceStyle, PyVolumeStyle,
};
use pyo3::prelude::*;

#[pyclass(name = "RepresentationKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyRepresentationKind {
    Spacefill,
    BallAndStick,
    Licorice,
    Lines,
    Cartoon,
    Trace,
    Tube,
    Surface,
    Volume,
    Segmentation,
    Beads,
    Rocket,
    Twister,
    PaperChain,
    Points,
}

impl From<PyRepresentationKind> for pdviewx::RepresentationKind {
    fn from(value: PyRepresentationKind) -> Self {
        match value {
            PyRepresentationKind::Spacefill => Self::Spacefill,
            PyRepresentationKind::BallAndStick => Self::BallAndStick,
            PyRepresentationKind::Licorice => Self::Licorice,
            PyRepresentationKind::Lines => Self::Lines,
            PyRepresentationKind::Cartoon => Self::Cartoon,
            PyRepresentationKind::Trace => Self::Trace,
            PyRepresentationKind::Tube => Self::Tube,
            PyRepresentationKind::Surface => Self::Surface,
            PyRepresentationKind::Volume => Self::Volume,
            PyRepresentationKind::Segmentation => Self::Segmentation,
            PyRepresentationKind::Beads => Self::Beads,
            PyRepresentationKind::Rocket => Self::Rocket,
            PyRepresentationKind::Twister => Self::Twister,
            PyRepresentationKind::PaperChain => Self::PaperChain,
            PyRepresentationKind::Points => Self::Points,
        }
    }
}

impl From<pdviewx::RepresentationKind> for PyRepresentationKind {
    fn from(value: pdviewx::RepresentationKind) -> Self {
        match value {
            pdviewx::RepresentationKind::Spacefill => Self::Spacefill,
            pdviewx::RepresentationKind::BallAndStick => Self::BallAndStick,
            pdviewx::RepresentationKind::Licorice => Self::Licorice,
            pdviewx::RepresentationKind::Lines => Self::Lines,
            pdviewx::RepresentationKind::Cartoon => Self::Cartoon,
            pdviewx::RepresentationKind::Trace => Self::Trace,
            pdviewx::RepresentationKind::Tube => Self::Tube,
            pdviewx::RepresentationKind::Surface => Self::Surface,
            pdviewx::RepresentationKind::Volume => Self::Volume,
            pdviewx::RepresentationKind::Segmentation => Self::Segmentation,
            pdviewx::RepresentationKind::Beads => Self::Beads,
            pdviewx::RepresentationKind::Rocket => Self::Rocket,
            pdviewx::RepresentationKind::Twister => Self::Twister,
            pdviewx::RepresentationKind::PaperChain => Self::PaperChain,
            _ => Self::Points,
        }
    }
}

#[pyclass(name = "Representation", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRepresentation {
    pub(crate) config: pdviewx::RepresentationConfig,
}

impl PyRepresentation {
    pub(crate) fn new(kind: pdviewx::RepresentationKind) -> Self {
        Self {
            config: pdviewx::RepresentationConfig::new(kind),
        }
    }
    pub(crate) fn kind_value(&self) -> PyRepresentationKind {
        self.config.kind().into()
    }
}

#[pymethods]
impl PyRepresentation {
    #[staticmethod]
    fn spacefill() -> Self {
        Self::new(pdviewx::RepresentationKind::Spacefill)
    }
    #[staticmethod]
    fn ball_and_stick() -> Self {
        Self::new(pdviewx::RepresentationKind::BallAndStick)
    }
    #[staticmethod]
    fn licorice() -> Self {
        Self::new(pdviewx::RepresentationKind::Licorice)
    }
    #[staticmethod]
    fn lines() -> Self {
        Self::new(pdviewx::RepresentationKind::Lines)
    }
    #[staticmethod]
    fn cartoon() -> Self {
        Self::new(pdviewx::RepresentationKind::Cartoon)
    }
    #[staticmethod]
    fn trace() -> Self {
        Self::new(pdviewx::RepresentationKind::Trace)
    }
    #[staticmethod]
    fn tube() -> Self {
        Self::new(pdviewx::RepresentationKind::Tube)
    }
    #[staticmethod]
    fn surface() -> Self {
        Self::new(pdviewx::RepresentationKind::Surface)
    }
    #[staticmethod]
    fn volume() -> Self {
        Self::new(pdviewx::RepresentationKind::Volume)
    }
    #[staticmethod]
    fn segmentation() -> Self {
        Self::new(pdviewx::RepresentationKind::Segmentation)
    }
    #[staticmethod]
    fn beads() -> Self {
        Self::new(pdviewx::RepresentationKind::Beads)
    }
    #[staticmethod]
    fn rocket() -> Self {
        Self::new(pdviewx::RepresentationKind::Rocket)
    }
    #[staticmethod]
    fn twister() -> Self {
        Self::new(pdviewx::RepresentationKind::Twister)
    }
    #[staticmethod]
    fn paper_chain() -> Self {
        Self::new(pdviewx::RepresentationKind::PaperChain)
    }
    #[staticmethod]
    fn points() -> Self {
        Self::new(pdviewx::RepresentationKind::Points)
    }

    fn color(&self, color: PyColorScheme) -> Self {
        Self {
            config: self.config.clone().color(color.0),
        }
    }
    fn material(&self, material: PyMaterial) -> Self {
        Self {
            config: self.config.clone().material(material.0),
        }
    }
    fn clipping(&self, clipping: PyClipSet) -> Self {
        Self {
            config: self.config.clone().clipping(clipping.0),
        }
    }
    fn radius_scale(&self, scale: f32) -> Self {
        Self {
            config: self.config.clone().radius_scale(scale),
        }
    }
    fn bond_radius(&self, radius: f32) -> Self {
        Self {
            config: self.config.clone().bond_radius(radius),
        }
    }
    fn isolevel(&self, level: f32) -> Self {
        Self {
            config: self.config.clone().isolevel(level),
        }
    }
    fn surface_presentation(&self, kind: PySurfaceKind, style: PySurfaceStyle) -> Self {
        Self {
            config: self.config.clone().surface(kind.into(), style.into()),
        }
    }
    fn volume_style(&self, style: PyVolumeStyle) -> Self {
        Self {
            config: self.config.clone().volume_style(style.0),
        }
    }
    fn segmentation_style(&self, style: PySegmentationStyle) -> Self {
        Self {
            config: self.config.clone().segmentation_style(style.0),
        }
    }
    fn surface_scalar(&self, overlay: PySurfaceScalarOverlay) -> Self {
        Self {
            config: self.config.clone().surface_scalar(overlay.0),
        }
    }

    #[getter]
    fn kind(&self) -> PyRepresentationKind {
        self.kind_value()
    }
    fn __repr__(&self) -> String {
        format!("Representation.{:?}", self.kind_value())
    }
}

#[pyclass(name = "RepresentationPreset", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRepresentationPreset(pub(crate) pdviewx::RepresentationPreset);

#[pymethods]
impl PyRepresentationPreset {
    #[staticmethod]
    fn cpk() -> Self {
        Self(pdviewx::RepresentationPreset::Cpk)
    }
    #[staticmethod]
    fn licorice() -> Self {
        Self(pdviewx::RepresentationPreset::Licorice)
    }
    #[staticmethod]
    fn paper_chain() -> Self {
        Self(pdviewx::RepresentationPreset::PaperChain)
    }
    #[staticmethod]
    fn dotted_solvent() -> Self {
        Self(pdviewx::RepresentationPreset::DottedSolvent)
    }
    #[staticmethod]
    fn signed_isosurface(
        negative_level: f32,
        positive_level: f32,
        negative_color: PyRgba8,
        positive_color: PyRgba8,
    ) -> Self {
        Self(pdviewx::RepresentationPreset::SignedIsosurface {
            negative_level,
            positive_level,
            negative_color: negative_color.0,
            positive_color: positive_color.0,
        })
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRepresentationKind>()?;
    module.add_class::<PyRepresentation>()?;
    module.add_class::<PyRepresentationPreset>()
}

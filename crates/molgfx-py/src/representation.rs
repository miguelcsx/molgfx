//! Python adapters for declarative representation descriptors.

use super::PyMaterial;
use crate::math::PyRgba8;
use crate::values::{
    PyClipSet, PyColorScheme, PySegmentationStyle, PySurfaceKind, PySurfaceScalarOverlay,
    PySurfaceStyle, PyVolumeStyle,
};
use crate::visual::PyVisualStyle;
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

impl From<PyRepresentationKind> for molgfx::RepresentationKind {
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

impl From<molgfx::RepresentationKind> for PyRepresentationKind {
    fn from(value: molgfx::RepresentationKind) -> Self {
        match value {
            molgfx::RepresentationKind::Spacefill => Self::Spacefill,
            molgfx::RepresentationKind::BallAndStick => Self::BallAndStick,
            molgfx::RepresentationKind::Licorice => Self::Licorice,
            molgfx::RepresentationKind::Lines => Self::Lines,
            molgfx::RepresentationKind::Cartoon => Self::Cartoon,
            molgfx::RepresentationKind::Trace => Self::Trace,
            molgfx::RepresentationKind::Tube => Self::Tube,
            molgfx::RepresentationKind::Surface => Self::Surface,
            molgfx::RepresentationKind::Volume => Self::Volume,
            molgfx::RepresentationKind::Segmentation => Self::Segmentation,
            molgfx::RepresentationKind::Beads => Self::Beads,
            molgfx::RepresentationKind::Rocket => Self::Rocket,
            molgfx::RepresentationKind::Twister => Self::Twister,
            molgfx::RepresentationKind::PaperChain => Self::PaperChain,
            molgfx::RepresentationKind::Points => Self::Points,
        }
    }
}

#[pyclass(name = "Representation", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRepresentation {
    pub(crate) config: molgfx::RepresentationConfig,
}

impl PyRepresentation {
    pub(crate) fn new(kind: molgfx::RepresentationKind) -> Self {
        Self {
            config: molgfx::RepresentationConfig::new(kind),
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
        Self::new(molgfx::RepresentationKind::Spacefill)
    }
    #[staticmethod]
    fn ball_and_stick() -> Self {
        Self::new(molgfx::RepresentationKind::BallAndStick)
    }
    #[staticmethod]
    fn licorice() -> Self {
        Self::new(molgfx::RepresentationKind::Licorice)
    }
    #[staticmethod]
    fn lines() -> Self {
        Self::new(molgfx::RepresentationKind::Lines)
    }
    #[staticmethod]
    fn cartoon() -> Self {
        Self::new(molgfx::RepresentationKind::Cartoon)
    }
    #[staticmethod]
    fn trace() -> Self {
        Self::new(molgfx::RepresentationKind::Trace)
    }
    #[staticmethod]
    fn tube() -> Self {
        Self::new(molgfx::RepresentationKind::Tube)
    }
    #[staticmethod]
    fn surface() -> Self {
        Self::new(molgfx::RepresentationKind::Surface)
    }
    #[staticmethod]
    fn volume() -> Self {
        Self::new(molgfx::RepresentationKind::Volume)
    }
    #[staticmethod]
    fn segmentation() -> Self {
        Self::new(molgfx::RepresentationKind::Segmentation)
    }
    #[staticmethod]
    fn beads() -> Self {
        Self::new(molgfx::RepresentationKind::Beads)
    }
    #[staticmethod]
    fn rocket() -> Self {
        Self::new(molgfx::RepresentationKind::Rocket)
    }
    #[staticmethod]
    fn twister() -> Self {
        Self::new(molgfx::RepresentationKind::Twister)
    }
    #[staticmethod]
    fn paper_chain() -> Self {
        Self::new(molgfx::RepresentationKind::PaperChain)
    }
    #[staticmethod]
    fn points() -> Self {
        Self::new(molgfx::RepresentationKind::Points)
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
    fn visual(&self, style: PyVisualStyle) -> Self {
        Self {
            config: self.config.clone().visual(style.0),
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
pub(crate) struct PyRepresentationPreset(pub(crate) molgfx::RepresentationPreset);

#[pymethods]
impl PyRepresentationPreset {
    #[staticmethod]
    fn cpk() -> Self {
        Self(molgfx::RepresentationPreset::Cpk)
    }
    #[staticmethod]
    fn licorice() -> Self {
        Self(molgfx::RepresentationPreset::Licorice)
    }
    #[staticmethod]
    fn paper_chain() -> Self {
        Self(molgfx::RepresentationPreset::PaperChain)
    }
    #[staticmethod]
    fn dotted_solvent() -> Self {
        Self(molgfx::RepresentationPreset::DottedSolvent)
    }
    #[staticmethod]
    fn signed_isosurface(
        negative_level: f32,
        positive_level: f32,
        negative_color: PyRgba8,
        positive_color: PyRgba8,
    ) -> Self {
        Self(molgfx::RepresentationPreset::SignedIsosurface {
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

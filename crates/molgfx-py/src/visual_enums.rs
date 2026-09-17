//! Visual-program enums mirrored without Python-side behavior.

use pyo3::prelude::*;

#[pyclass(name = "VisualOutput", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyVisualOutput {
    BaseColor,
    Opacity,
    Emission,
    Roughness,
    Specular,
    MaterialStrength,
    Visibility,
    SilhouetteSoftness,
    RadiusScale,
    WidthScale,
    PositionOffset,
}

impl From<PyVisualOutput> for molgfx::VisualOutput {
    fn from(value: PyVisualOutput) -> Self {
        match value {
            PyVisualOutput::BaseColor => Self::BaseColor,
            PyVisualOutput::Opacity => Self::Opacity,
            PyVisualOutput::Emission => Self::Emission,
            PyVisualOutput::Roughness => Self::Roughness,
            PyVisualOutput::Specular => Self::Specular,
            PyVisualOutput::MaterialStrength => Self::MaterialStrength,
            PyVisualOutput::Visibility => Self::Visibility,
            PyVisualOutput::SilhouetteSoftness => Self::SilhouetteSoftness,
            PyVisualOutput::RadiusScale => Self::RadiusScale,
            PyVisualOutput::WidthScale => Self::WidthScale,
            PyVisualOutput::PositionOffset => Self::PositionOffset,
        }
    }
}

#[pyclass(name = "VisualStage", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyVisualStage {
    Uniform,
    Entity,
    Fragment,
}

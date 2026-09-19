//! Display color and tone-mapping policy.

use pyo3::prelude::*;

#[pyclass(name = "ToneMapping", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyToneMapping {
    AcesFitted,
    Reinhard,
    /// The identity curve. Named `Disabled` rather than `None` because
    /// `ToneMapping.None` is a syntax error in Python source.
    Disabled,
}

impl From<PyToneMapping> for molgfx::render::ToneMapping {
    fn from(value: PyToneMapping) -> Self {
        match value {
            PyToneMapping::AcesFitted => Self::AcesFitted,
            PyToneMapping::Reinhard => Self::Reinhard,
            PyToneMapping::Disabled => Self::None,
        }
    }
}

impl From<molgfx::render::ToneMapping> for PyToneMapping {
    fn from(value: molgfx::render::ToneMapping) -> Self {
        match value {
            molgfx::render::ToneMapping::AcesFitted => Self::AcesFitted,
            molgfx::render::ToneMapping::Reinhard => Self::Reinhard,
            molgfx::render::ToneMapping::None => Self::Disabled,
        }
    }
}

#[pyclass(name = "DisplayGamut", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyDisplayGamut {
    Srgb,
    DisplayP3,
    Rec2020,
}

impl From<PyDisplayGamut> for molgfx::render::DisplayGamut {
    fn from(value: PyDisplayGamut) -> Self {
        match value {
            PyDisplayGamut::Srgb => Self::Srgb,
            PyDisplayGamut::DisplayP3 => Self::DisplayP3,
            PyDisplayGamut::Rec2020 => Self::Rec2020,
        }
    }
}

impl From<molgfx::render::DisplayGamut> for PyDisplayGamut {
    fn from(value: molgfx::render::DisplayGamut) -> Self {
        match value {
            molgfx::render::DisplayGamut::Srgb => Self::Srgb,
            molgfx::render::DisplayGamut::DisplayP3 => Self::DisplayP3,
            molgfx::render::DisplayGamut::Rec2020 => Self::Rec2020,
        }
    }
}

#[pyclass(name = "TransferFunction", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyTransferFunction {
    Srgb,
    Linear,
    Pq,
    Hlg,
}

impl From<PyTransferFunction> for molgfx::render::TransferFunction {
    fn from(value: PyTransferFunction) -> Self {
        match value {
            PyTransferFunction::Srgb => Self::Srgb,
            PyTransferFunction::Linear => Self::Linear,
            PyTransferFunction::Pq => Self::Pq,
            PyTransferFunction::Hlg => Self::Hlg,
        }
    }
}

impl From<molgfx::render::TransferFunction> for PyTransferFunction {
    fn from(value: molgfx::render::TransferFunction) -> Self {
        match value {
            molgfx::render::TransferFunction::Srgb => Self::Srgb,
            molgfx::render::TransferFunction::Linear => Self::Linear,
            molgfx::render::TransferFunction::Pq => Self::Pq,
            molgfx::render::TransferFunction::Hlg => Self::Hlg,
        }
    }
}

#[pyclass(name = "DisplayTransform", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDisplayTransform(pub(crate) molgfx::render::DisplayTransform);

#[pymethods]
impl PyDisplayTransform {
    #[new]
    #[pyo3(signature = (exposure_ev=0.0, contrast=1.0, saturation=1.0, vignette_strength=0.0, tone_mapping=None, gamut=None, transfer=None, peak_luminance_nits=100.0))]
    fn new(
        exposure_ev: f32,
        contrast: f32,
        saturation: f32,
        vignette_strength: f32,
        tone_mapping: Option<PyToneMapping>,
        gamut: Option<PyDisplayGamut>,
        transfer: Option<PyTransferFunction>,
        peak_luminance_nits: f32,
    ) -> Self {
        let default = molgfx::render::DisplayTransform::default();
        Self(molgfx::render::DisplayTransform {
            exposure_ev,
            contrast,
            saturation,
            vignette_strength,
            tone_mapping: tone_mapping.map_or(default.tone_mapping, Into::into),
            gamut: gamut.map_or(default.gamut, Into::into),
            transfer: transfer.map_or(default.transfer, Into::into),
            peak_luminance_nits,
        })
    }

    #[staticmethod]
    fn cinematic() -> Self {
        Self(molgfx::render::DisplayTransform::cinematic())
    }

    #[getter]
    fn exposure_ev(&self) -> f32 {
        self.0.exposure_ev
    }

    #[getter]
    fn contrast(&self) -> f32 {
        self.0.contrast
    }

    #[getter]
    fn saturation(&self) -> f32 {
        self.0.saturation
    }

    #[getter]
    fn vignette_strength(&self) -> f32 {
        self.0.vignette_strength
    }

    #[getter]
    fn tone_mapping(&self) -> PyToneMapping {
        self.0.tone_mapping.into()
    }

    #[getter]
    fn gamut(&self) -> PyDisplayGamut {
        self.0.gamut.into()
    }

    #[getter]
    fn transfer(&self) -> PyTransferFunction {
        self.0.transfer.into()
    }

    #[getter]
    fn peak_luminance_nits(&self) -> f32 {
        self.0.peak_luminance_nits
    }
}

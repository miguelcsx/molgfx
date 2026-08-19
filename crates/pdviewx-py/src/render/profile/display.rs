//! Display color and tone-mapping policy.

use pyo3::prelude::*;

#[pyclass(name = "ToneMapping", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyToneMapping {
    AcesFitted,
    Reinhard,
    None,
}

impl From<PyToneMapping> for pdviewx::ToneMapping {
    fn from(value: PyToneMapping) -> Self {
        match value {
            PyToneMapping::AcesFitted => Self::AcesFitted,
            PyToneMapping::Reinhard => Self::Reinhard,
            PyToneMapping::None => Self::None,
        }
    }
}

impl From<pdviewx::ToneMapping> for PyToneMapping {
    fn from(value: pdviewx::ToneMapping) -> Self {
        match value {
            pdviewx::ToneMapping::AcesFitted => Self::AcesFitted,
            pdviewx::ToneMapping::Reinhard => Self::Reinhard,
            pdviewx::ToneMapping::None => Self::None,
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

impl From<PyDisplayGamut> for pdviewx::DisplayGamut {
    fn from(value: PyDisplayGamut) -> Self {
        match value {
            PyDisplayGamut::Srgb => Self::Srgb,
            PyDisplayGamut::DisplayP3 => Self::DisplayP3,
            PyDisplayGamut::Rec2020 => Self::Rec2020,
        }
    }
}

impl From<pdviewx::DisplayGamut> for PyDisplayGamut {
    fn from(value: pdviewx::DisplayGamut) -> Self {
        match value {
            pdviewx::DisplayGamut::Srgb => Self::Srgb,
            pdviewx::DisplayGamut::DisplayP3 => Self::DisplayP3,
            pdviewx::DisplayGamut::Rec2020 => Self::Rec2020,
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

impl From<PyTransferFunction> for pdviewx::TransferFunction {
    fn from(value: PyTransferFunction) -> Self {
        match value {
            PyTransferFunction::Srgb => Self::Srgb,
            PyTransferFunction::Linear => Self::Linear,
            PyTransferFunction::Pq => Self::Pq,
            PyTransferFunction::Hlg => Self::Hlg,
        }
    }
}

impl From<pdviewx::TransferFunction> for PyTransferFunction {
    fn from(value: pdviewx::TransferFunction) -> Self {
        match value {
            pdviewx::TransferFunction::Srgb => Self::Srgb,
            pdviewx::TransferFunction::Linear => Self::Linear,
            pdviewx::TransferFunction::Pq => Self::Pq,
            pdviewx::TransferFunction::Hlg => Self::Hlg,
        }
    }
}

#[pyclass(name = "DisplayTransform", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDisplayTransform(pub(crate) pdviewx::DisplayTransform);

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
        let default = pdviewx::DisplayTransform::default();
        Self(pdviewx::DisplayTransform {
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
        Self(pdviewx::DisplayTransform::cinematic())
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyToneMapping>()?;
    module.add_class::<PyDisplayGamut>()?;
    module.add_class::<PyTransferFunction>()?;
    module.add_class::<PyDisplayTransform>()
}

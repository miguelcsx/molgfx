//! Typed scalar-grid presentations sharing one upload.

use super::{PyDataSource, rgb};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[derive(Clone, Debug)]
#[pyclass(name = "Volume", frozen, skip_from_py_object)]
pub(super) struct PyVolume(pub(super) molgfx::VolumeSpec);

#[pyfunction]
#[pyo3(signature = (*, source, dimensions, voxel_to_world=None))]
pub(super) fn volume(
    source: &PyDataSource,
    dimensions: (u32, u32, u32),
    voxel_to_world: Option<[f32; 16]>,
) -> PyVolume {
    let mut affine = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    if let Some(matrix) = voxel_to_world {
        affine = matrix;
    }
    PyVolume(molgfx::VolumeSpec {
        source: source.0.clone(),
        dimensions: [dimensions.0, dimensions.1, dimensions.2],
        voxel_to_world: affine,
        presentations: Vec::new(),
        region: None,
    })
}

#[pymethods]
impl PyVolume {
    /// Adds gradient streamlines integrated in world space from this scalar grid.
    #[pyo3(signature = (**options))]
    fn field_lines(&self, options: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut controls = molgfx::FieldLines::default();
        if let Some(options) = options {
            for (key, value) in options {
                let key: String = key.extract()?;
                match key.as_str() {
                    "seed_stride" => controls.seed_stride = value.extract()?,
                    "seed_gradient" => controls.seed_gradient = value.extract()?,
                    "step" => controls.step = value.extract()?,
                    "length" => controls.length = value.extract()?,
                    "critical_gradient" => controls.critical_gradient = value.extract()?,
                    "max_steps" => controls.max_steps = value.extract()?,
                    "max_segments" => controls.max_segments = value.extract()?,
                    "width_pixels" => controls.width_pixels = value.extract()?,
                    "color" => controls.color = rgb(value.extract()?),
                    "opacity" => controls.opacity = value.extract()?,
                    _ => {
                        return Err(PyTypeError::new_err(format!(
                            "unknown field-line control {key}"
                        )));
                    }
                }
            }
        }
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::FieldLines { options: controls });
        Ok(value)
    }

    /// Draws both signs of a sampled wavefunction amplitude with shared geometry controls.
    #[pyo3(signature = (amplitude, *, positive=(255,0,0), negative=(0,0,255), opacity=1.0, style="solid", width=0.1))]
    fn orbital(
        &self,
        amplitude: f32,
        positive: (u8, u8, u8),
        negative: (u8, u8, u8),
        opacity: f32,
        style: &str,
        width: f32,
    ) -> PyResult<Self> {
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::Orbital {
                amplitude,
                positive: rgb(positive),
                negative: rgb(negative),
                opacity,
                style: iso_style(style, width)?,
            });
        Ok(value)
    }
    #[pyo3(signature = (isovalue, *, color=(49,104,142), opacity=1.0, style="solid", width=0.1))]
    fn isosurface(
        &self,
        isovalue: f32,
        color: (u8, u8, u8),
        opacity: f32,
        style: &str,
        width: f32,
    ) -> PyResult<Self> {
        let style = iso_style(style, width)?;
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::Isosurface {
                isovalue,
                color: rgb(color),
                opacity,
                style,
            });
        Ok(value)
    }

    #[pyo3(signature = (transfer, *, opacity_scale=2.0, step_scale=0.65, medium=false))]
    fn direct(
        &self,
        transfer: Vec<(f32, (u8, u8, u8), f32)>,
        opacity_scale: f32,
        step_scale: f32,
        medium: bool,
    ) -> Self {
        let transfer = transfer
            .into_iter()
            .map(|(value, color, opacity)| molgfx::VolumeTransferPoint {
                value,
                color: rgb(color),
                opacity,
            })
            .collect();
        let presentation = if medium {
            molgfx::VolumePresentation::Medium {
                transfer,
                opacity_scale,
                step_scale,
            }
        } else {
            molgfx::VolumePresentation::Direct {
                transfer,
                opacity_scale,
                step_scale,
            }
        };
        let mut value = self.clone();
        value.0.presentations.push(presentation);
        value
    }

    #[pyo3(signature = (*, point, normal, ramp="viridis", domain=(0.0,1.0)))]
    fn slice(&self, point: [f32; 3], normal: [f32; 3], ramp: &str, domain: (f32, f32)) -> Self {
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::Slice {
                point,
                normal,
                ramp: ramp.into(),
                domain: [domain.0, domain.1],
            });
        value
    }

    #[pyo3(signature = (isovalue, *, color=(49,104,142), opacity=1.0))]
    fn liquid_surface(&self, isovalue: f32, color: (u8, u8, u8), opacity: f32) -> Self {
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::LiquidSurface {
                isovalue,
                color: rgb(color),
                opacity,
            });
        value
    }

    fn region(&self, minimum: [u32; 3], maximum: [u32; 3]) -> Self {
        let mut value = self.clone();
        value.0.region = Some(molgfx::VolumeRegion { minimum, maximum });
        value
    }
}

fn iso_style(style: &str, width: f32) -> PyResult<molgfx::IsoStyle> {
    match style {
        "solid" => Ok(molgfx::IsoStyle::Solid),
        "mesh" => Ok(molgfx::IsoStyle::Mesh {
            line_width_voxels: width,
        }),
        "dots" => Ok(molgfx::IsoStyle::Dots {
            dot_radius_voxels: width,
        }),
        _ => Err(PyTypeError::new_err(
            "boundary style must be solid, mesh, or dots",
        )),
    }
}

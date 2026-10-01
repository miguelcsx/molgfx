//! The pocket-and-pose composition's style value.

use molgfx::preset::PocketStyle;
use pyo3::prelude::*;

/// Distances, opacities and colours of the pocket-and-pose composition.
///
/// Immutable: each `with_*` method returns a changed copy. Validation happens
/// when the style is applied to a scene, where the scene layer owns the rules.
#[derive(Clone, Copy, Debug)]
#[pyclass(name = "PocketStyle", frozen, from_py_object)]
pub(super) struct PyPocketStyle(pub(super) PocketStyle);

fn rgb(value: (u8, u8, u8)) -> molgfx::Color {
    molgfx::Color::rgb(value.0, value.1, value.2)
}

#[pymethods]
impl PyPocketStyle {
    #[new]
    #[pyo3(signature = (*, near=None, mid=None))]
    fn new(near: Option<f32>, mid: Option<f32>) -> Self {
        let mut style = PocketStyle::default();
        if let Some(near) = near {
            style.near = near;
        }
        if let Some(mid) = mid {
            style.mid = mid;
        }
        Self(style)
    }

    #[getter]
    const fn near(&self) -> f32 {
        self.0.near
    }

    #[getter]
    const fn mid(&self) -> f32 {
        self.0.mid
    }

    #[getter]
    const fn pocket_opacity(&self) -> f32 {
        self.0.pocket_opacity
    }

    #[getter]
    const fn context_opacity(&self) -> f32 {
        self.0.context_opacity
    }

    #[getter]
    const fn solvent_opacity(&self) -> f32 {
        self.0.solvent_opacity
    }

    /// A copy with the given opacities; unnamed ones keep their value.
    #[pyo3(signature = (*, pocket=None, context=None, solvent=None))]
    fn with_opacity(
        &self,
        pocket: Option<f32>,
        context: Option<f32>,
        solvent: Option<f32>,
    ) -> Self {
        let mut style = self.0;
        if let Some(opacity) = pocket {
            style.pocket_opacity = opacity;
        }
        if let Some(opacity) = context {
            style.context_opacity = opacity;
        }
        if let Some(opacity) = solvent {
            style.solvent_opacity = opacity;
        }
        Self(style)
    }

    /// A copy with the given `(r, g, b)` colours; unnamed ones keep their value.
    #[pyo3(signature = (*, pocket=None, context=None, solvent=None))]
    fn with_colors(
        &self,
        pocket: Option<(u8, u8, u8)>,
        context: Option<(u8, u8, u8)>,
        solvent: Option<(u8, u8, u8)>,
    ) -> Self {
        let mut style = self.0;
        style.pocket_color = pocket.map_or(style.pocket_color, rgb);
        style.context_color = context.map_or(style.context_color, rgb);
        style.solvent_color = solvent.map_or(style.solvent_color, rgb);
        Self(style)
    }

    fn __repr__(&self) -> String {
        format!("PocketStyle(near={}, mid={})", self.0.near, self.0.mid)
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPocketStyle>()
}

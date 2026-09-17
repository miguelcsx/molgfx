//! Python scene methods lowering directly into Rust generic compositions.

use super::PyGenericCompositionView;
use crate::{
    core::{PyAttributeHandle, PyRowDomain, PyScene},
    error::SemanticError,
    math::PyRgba8,
    values::PyScalarRamp,
};
use pyo3::prelude::*;

#[pymethods]
impl PyScene {
    #[pyo3(signature = (domain, emphasis, context_opacity=0.16, focus_opacity=1.0, order=0))]
    fn compose_focus(
        &mut self,
        domain: PyRowDomain,
        emphasis: PyAttributeHandle,
        context_opacity: f32,
        focus_opacity: f32,
        order: i32,
    ) -> PyResult<PyGenericCompositionView> {
        composition(molgfx::GenericCompositionScene::compose_focus(
            &mut self.inner,
            molgfx::FocusLayer {
                domain: domain.0,
                emphasis: emphasis.0,
            },
            molgfx::FocusCompositionStyle {
                context_opacity,
                focus_opacity,
                order,
            },
        ))
        .map(Into::into)
    }

    #[pyo3(signature = (domains, deltas, context_threshold, emphasis_threshold, ramp, context_opacity=0.16, order=0))]
    #[allow(clippy::too_many_arguments)]
    fn compose_difference(
        &mut self,
        domains: Vec<PyRowDomain>,
        deltas: Vec<PyAttributeHandle>,
        context_threshold: f32,
        emphasis_threshold: f32,
        ramp: PyScalarRamp,
        context_opacity: f32,
        order: i32,
    ) -> PyResult<PyGenericCompositionView> {
        if domains.len() != deltas.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "domains and deltas must be aligned",
            ));
        }
        let layers = domains
            .into_iter()
            .zip(deltas)
            .map(|(domain, delta)| molgfx::DifferenceLayer {
                domain: domain.0,
                delta: delta.0,
            })
            .collect::<Vec<_>>();
        composition(molgfx::GenericCompositionScene::compose_difference(
            &mut self.inner,
            &layers,
            molgfx::DifferenceCompositionStyle {
                context_threshold,
                emphasis_threshold,
                context_opacity,
                ramp: ramp.0,
                order,
            },
        ))
        .map(Into::into)
    }

    #[pyo3(signature = (domains, weights, colors, dominant_opacity=1.0, alternate_opacity=0.55, minimum_opacity=0.08, order=0))]
    #[allow(clippy::too_many_arguments)]
    fn compose_ensemble(
        &mut self,
        domains: Vec<PyRowDomain>,
        weights: Vec<f32>,
        colors: Vec<PyRgba8>,
        dominant_opacity: f32,
        alternate_opacity: f32,
        minimum_opacity: f32,
        order: i32,
    ) -> PyResult<PyGenericCompositionView> {
        if domains.len() != weights.len() || domains.len() != colors.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "domains, weights and colors must be aligned",
            ));
        }
        let layers = domains
            .into_iter()
            .zip(weights)
            .zip(colors)
            .map(|((domain, weight), color)| molgfx::EnsembleLayer {
                domain: domain.0,
                weight,
                color: color.0,
            })
            .collect::<Vec<_>>();
        composition(molgfx::GenericCompositionScene::compose_ensemble(
            &mut self.inner,
            &layers,
            molgfx::EnsembleCompositionStyle {
                dominant_opacity,
                alternate_opacity,
                minimum_opacity,
                order,
            },
        ))
        .map(Into::into)
    }
}

fn composition<T>(result: Result<T, molgfx::CompositionError>) -> PyResult<T> {
    result.map_err(|error| SemanticError::new_err(error.to_string()))
}

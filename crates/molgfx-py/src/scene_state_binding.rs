//! Python adapters for renderer-independent interaction and view state.

use crate::binding::selection;
use crate::scene_binding::PyScene;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyAny;

#[pymethods]
impl PyScene {
    fn set_parameter(
        &mut self,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        parameter: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let id =
            molgfx::RepresentationId::new(crate::id_binding::representation_id(representation)?);
        if let Ok(parameter) =
            parameter.extract::<PyRef<'_, crate::visual_binding::PyScalarParameter>>()
        {
            return self.stage_or_apply(
                py,
                molgfx::schema::PatchOperation::SetParameter {
                    id,
                    name: parameter.0.name().into(),
                    value: Some(molgfx::ParameterValue::Scalar(value.extract::<f32>()?)),
                },
            );
        }
        if let Ok(parameter) =
            parameter.extract::<PyRef<'_, crate::visual_binding::PyVectorParameter>>()
        {
            let value = value.extract::<(f32, f32, f32)>()?;
            return self.stage_or_apply(
                py,
                molgfx::schema::PatchOperation::SetParameter {
                    id,
                    name: parameter.0.name().into(),
                    value: Some(molgfx::ParameterValue::Vector([value.0, value.1, value.2])),
                },
            );
        }
        if let Ok(parameter) =
            parameter.extract::<PyRef<'_, crate::visual_binding::PyColorParameter>>()
        {
            let value = value.extract::<(u8, u8, u8)>()?;
            return self.stage_or_apply(
                py,
                molgfx::schema::PatchOperation::SetParameter {
                    id,
                    name: parameter.0.name().into(),
                    value: Some(molgfx::ParameterValue::Color(molgfx::Color::rgb(
                        value.0, value.1, value.2,
                    ))),
                },
            );
        }
        Err(PyValueError::new_err(
            "expected a scalar, vector, or color parameter",
        ))
    }

    fn set_camera(
        &mut self,
        py: Python<'_>,
        camera: Option<&crate::authoring_binding::PyCamera>,
    ) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetCamera {
                camera: camera.map(|value| value.0),
            },
        )
    }

    #[pyo3(signature = (*, channel, target=None, name=None))]
    fn set_interaction(
        &mut self,
        py: Python<'_>,
        channel: &str,
        target: Option<&Bound<'_, PyAny>>,
        name: Option<&str>,
    ) -> PyResult<()> {
        let channel = match channel {
            "selected" => molgfx::InteractionChannel::Selected,
            "hovered" => molgfx::InteractionChannel::Hovered,
            "focused" => molgfx::InteractionChannel::Focused,
            "muted" => molgfx::InteractionChannel::Muted,
            "hidden" => molgfx::InteractionChannel::Hidden,
            "custom" => molgfx::InteractionChannel::Custom(
                name.filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| PyValueError::new_err("custom channels require a name"))?
                    .into(),
            ),
            _ => return Err(PyValueError::new_err("unknown interaction channel")),
        };
        let target = target.map(selection).transpose()?.map(Into::into);
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetInteraction {
                channel,
                selection: target,
            },
        )
    }
}

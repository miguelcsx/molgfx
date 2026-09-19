//! Backdrop, lighting, and illustration context.

use crate::math::{PyRgba8, PyVec3};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

struct LightingArguments<'py> {
    args: &'py Bound<'py, PyTuple>,
    kwargs: Option<&'py Bound<'py, PyDict>>,
    next: usize,
}

impl<'py> LightingArguments<'py> {
    const NAMES: [&'static str; 15] = [
        "zenith",
        "horizon",
        "ground",
        "rim_color",
        "key_color",
        "fill_color",
        "key_direction",
        "fill_direction",
        "diffuse_strength",
        "specular_strength",
        "rim_strength",
        "key_strength",
        "fill_strength",
        "key_angular_radius",
        "shadow_strength",
    ];

    fn new(
        args: &'py Bound<'py, PyTuple>,
        kwargs: Option<&'py Bound<'py, PyDict>>,
    ) -> PyResult<Self> {
        if args.len() > Self::NAMES.len() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "LightingEnvironment accepts at most 15 positional arguments",
            ));
        }
        if let Some(kwargs) = kwargs {
            for key in kwargs.keys() {
                let key: String = key.extract()?;
                if !Self::NAMES.contains(&key.as_str()) {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "LightingEnvironment got an unexpected keyword argument '{key}'"
                    )));
                }
            }
        }
        Ok(Self {
            args,
            kwargs,
            next: 0,
        })
    }

    fn raw(&mut self, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        let positional = if self.next < self.args.len() {
            let value = self.args.get_item(self.next)?;
            self.next += 1;
            Some(value)
        } else {
            None
        };
        let keyword = match self.kwargs {
            Some(kwargs) => kwargs.get_item(name)?,
            None => None,
        };
        if positional.is_some() && keyword.is_some() {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "LightingEnvironment got multiple values for '{name}'"
            )));
        }
        Ok(positional.or(keyword))
    }

    fn optional<T>(&mut self, name: &str) -> PyResult<Option<T>>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        let Some(value) = self.raw(name)? else {
            return Ok(None);
        };
        if value.is_none() {
            return Ok(None);
        }
        value.extract().map(Some).map_err(Into::into)
    }

    fn required<T>(&mut self, name: &str, default: T) -> PyResult<T>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        match self.optional(name)? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }
}

#[pyclass(name = "BackdropStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBackdropStyle(pub(crate) molgfx::render::BackdropStyle);

#[pymethods]
impl PyBackdropStyle {
    #[new]
    #[pyo3(signature = (top=None, bottom=None, glow_color=None, glow_strength=0.08))]
    fn new(
        top: Option<PyRgba8>,
        bottom: Option<PyRgba8>,
        glow_color: Option<PyRgba8>,
        glow_strength: f32,
    ) -> Self {
        let default = molgfx::render::BackdropStyle::default();
        Self(molgfx::render::BackdropStyle {
            top: top.map_or(default.top, |value| value.0),
            bottom: bottom.map_or(default.bottom, |value| value.0),
            glow_color: glow_color.map_or(default.glow_color, |value| value.0),
            glow_strength,
        })
    }

    #[staticmethod]
    fn compositing() -> Self {
        Self(molgfx::render::BackdropStyle::compositing())
    }

    #[staticmethod]
    fn transparent() -> Self {
        Self(molgfx::render::BackdropStyle::transparent())
    }

    #[staticmethod]
    fn studio() -> Self {
        Self(molgfx::render::BackdropStyle::studio())
    }

    #[getter]
    fn top(&self) -> PyRgba8 {
        self.0.top.into()
    }

    #[getter]
    fn bottom(&self) -> PyRgba8 {
        self.0.bottom.into()
    }

    #[getter]
    fn glow_color(&self) -> PyRgba8 {
        self.0.glow_color.into()
    }

    #[getter]
    fn glow_strength(&self) -> f32 {
        self.0.glow_strength
    }
}

#[pyclass(name = "LightingEnvironment", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyLightingEnvironment(pub(crate) molgfx::render::LightingEnvironment);

#[pymethods]
impl PyLightingEnvironment {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut arguments = LightingArguments::new(args, kwargs)?;
        let zenith: Option<PyRgba8> = arguments.optional("zenith")?;
        let horizon: Option<PyRgba8> = arguments.optional("horizon")?;
        let ground: Option<PyRgba8> = arguments.optional("ground")?;
        let rim_color: Option<PyRgba8> = arguments.optional("rim_color")?;
        let key_color: Option<PyRgba8> = arguments.optional("key_color")?;
        let fill_color: Option<PyRgba8> = arguments.optional("fill_color")?;
        let key_direction: Option<PyVec3> = arguments.optional("key_direction")?;
        let fill_direction: Option<PyVec3> = arguments.optional("fill_direction")?;
        let diffuse_strength = arguments.required("diffuse_strength", 0.72)?;
        let specular_strength = arguments.required("specular_strength", 0.58)?;
        let rim_strength = arguments.required("rim_strength", 0.22)?;
        let key_strength = arguments.required("key_strength", 1.55)?;
        let fill_strength = arguments.required("fill_strength", 0.28)?;
        let key_angular_radius = arguments.required("key_angular_radius", 0.16)?;
        let shadow_strength = arguments.required("shadow_strength", 0.48)?;
        let default = molgfx::render::LightingEnvironment::neutral();
        Ok(Self(molgfx::render::LightingEnvironment {
            zenith: zenith.map_or(default.zenith, |value| value.0),
            horizon: horizon.map_or(default.horizon, |value| value.0),
            ground: ground.map_or(default.ground, |value| value.0),
            rim_color: rim_color.map_or(default.rim_color, |value| value.0),
            key_color: key_color.map_or(default.key_color, |value| value.0),
            fill_color: fill_color.map_or(default.fill_color, |value| value.0),
            key_direction: key_direction.map_or(default.key_direction, |value| value.0),
            fill_direction: fill_direction.map_or(default.fill_direction, |value| value.0),
            diffuse_strength,
            specular_strength,
            rim_strength,
            key_strength,
            fill_strength,
            key_angular_radius,
            shadow_strength,
        }))
    }

    #[staticmethod]
    fn neutral() -> Self {
        Self(molgfx::render::LightingEnvironment::neutral())
    }

    #[staticmethod]
    fn documentary() -> Self {
        Self(molgfx::render::LightingEnvironment::documentary())
    }

    #[getter]
    fn zenith(&self) -> PyRgba8 {
        self.0.zenith.into()
    }

    #[getter]
    fn horizon(&self) -> PyRgba8 {
        self.0.horizon.into()
    }

    #[getter]
    fn ground(&self) -> PyRgba8 {
        self.0.ground.into()
    }

    #[getter]
    fn rim_color(&self) -> PyRgba8 {
        self.0.rim_color.into()
    }

    #[getter]
    fn key_color(&self) -> PyRgba8 {
        self.0.key_color.into()
    }

    #[getter]
    fn fill_color(&self) -> PyRgba8 {
        self.0.fill_color.into()
    }

    #[getter]
    fn key_direction(&self) -> PyVec3 {
        PyVec3(self.0.key_direction)
    }

    #[getter]
    fn fill_direction(&self) -> PyVec3 {
        PyVec3(self.0.fill_direction)
    }

    #[getter]
    fn diffuse_strength(&self) -> f32 {
        self.0.diffuse_strength
    }

    #[getter]
    fn specular_strength(&self) -> f32 {
        self.0.specular_strength
    }

    #[getter]
    fn rim_strength(&self) -> f32 {
        self.0.rim_strength
    }

    #[getter]
    fn key_strength(&self) -> f32 {
        self.0.key_strength
    }

    #[getter]
    fn fill_strength(&self) -> f32 {
        self.0.fill_strength
    }

    #[getter]
    fn key_angular_radius(&self) -> f32 {
        self.0.key_angular_radius
    }

    #[getter]
    fn shadow_strength(&self) -> f32 {
        self.0.shadow_strength
    }
}

#[pyclass(name = "IllustrationStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyIllustrationStyle(pub(crate) molgfx::render::IllustrationStyle);

#[pymethods]
impl PyIllustrationStyle {
    #[new]
    #[pyo3(signature = (
        silhouette_strength = 0.0,
        cavity_strength = 0.0,
        depth_cue_strength = 0.0,
        posterize_levels = 0.0,
        motion_persistence = 0.0,
        outline_width = 0.0,
    ))]
    fn new(
        silhouette_strength: f32,
        cavity_strength: f32,
        depth_cue_strength: f32,
        posterize_levels: f32,
        motion_persistence: f32,
        outline_width: f32,
    ) -> Self {
        Self(molgfx::render::IllustrationStyle {
            silhouette_strength,
            cavity_strength,
            depth_cue_strength,
            posterize_levels,
            motion_persistence,
            outline_width,
        })
    }

    #[staticmethod]
    fn publication() -> Self {
        Self(molgfx::render::IllustrationStyle::publication())
    }

    #[getter]
    fn silhouette_strength(&self) -> f32 {
        self.0.silhouette_strength
    }

    #[getter]
    fn cavity_strength(&self) -> f32 {
        self.0.cavity_strength
    }

    #[getter]
    fn depth_cue_strength(&self) -> f32 {
        self.0.depth_cue_strength
    }

    #[getter]
    fn posterize_levels(&self) -> f32 {
        self.0.posterize_levels
    }

    #[getter]
    fn motion_persistence(&self) -> f32 {
        self.0.motion_persistence
    }

    #[getter]
    fn outline_width(&self) -> f32 {
        self.0.outline_width
    }
}

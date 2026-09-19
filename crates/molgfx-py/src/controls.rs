//! Window-system-independent camera controls.

use crate::math::PyCamera;
use pyo3::prelude::*;

#[pyclass(name = "Button", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyButton {
    Left,
    Right,
    Middle,
}

impl From<PyButton> for molgfx::core::Button {
    fn from(value: PyButton) -> Self {
        match value {
            PyButton::Left => Self::Left,
            PyButton::Right => Self::Right,
            PyButton::Middle => Self::Middle,
        }
    }
}

#[pyclass(name = "Key", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyKey {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

impl From<PyKey> for molgfx::core::Key {
    fn from(value: PyKey) -> Self {
        match value {
            PyKey::Forward => Self::Forward,
            PyKey::Backward => Self::Backward,
            PyKey::Left => Self::Left,
            PyKey::Right => Self::Right,
            PyKey::Up => Self::Up,
            PyKey::Down => Self::Down,
        }
    }
}

#[pyclass(name = "InputEvent", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInputEvent(molgfx::core::InputEvent);

#[pymethods]
impl PyInputEvent {
    #[staticmethod]
    fn pointer_move(x: f32, y: f32) -> Self {
        Self(molgfx::core::InputEvent::PointerMove { x, y })
    }
    #[staticmethod]
    fn pointer_button(button: PyButton, pressed: bool, x: f32, y: f32) -> Self {
        Self(molgfx::core::InputEvent::PointerButton {
            button: button.into(),
            pressed,
            x,
            y,
        })
    }
    #[staticmethod]
    fn scroll(delta: f32) -> Self {
        Self(molgfx::core::InputEvent::Scroll { delta })
    }
    #[staticmethod]
    fn key(key: PyKey, pressed: bool) -> Self {
        Self(molgfx::core::InputEvent::Key {
            key: key.into(),
            pressed,
        })
    }
    #[staticmethod]
    fn pinch(scale: f32) -> Self {
        Self(molgfx::core::InputEvent::Pinch { scale })
    }
}

macro_rules! controller {
    ($python:literal, $name:ident, $native:ident) => {
        #[pyclass(name = $python, from_py_object)]
        #[derive(Clone, Copy, Debug, Default)]
        pub(crate) struct $name(molgfx::core::$native);

        #[pymethods]
        impl $name {
            #[new]
            fn new() -> Self {
                Self(molgfx::core::$native::default())
            }
            fn update(&mut self, event: PyInputEvent, camera: &mut PyCamera) {
                self.0.update(event.0, &mut camera.inner);
            }
        }
    };
}

controller!("ArcballController", PyArcballController, ArcballController);
controller!("OrbitController", PyOrbitController, OrbitController);
controller!("FlyController", PyFlyController, FlyController);

#[pymethods]
impl PyFlyController {
    fn advance(&self, camera: &mut PyCamera, dt: f32, speed: f32) {
        self.0.advance(&mut camera.inner, dt, speed);
    }
}

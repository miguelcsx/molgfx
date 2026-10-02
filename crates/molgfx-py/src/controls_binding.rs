//! Camera controllers: abstract input events in, a moved camera out.
//!
//! A host translates its toolkit's pointer, wheel and key events into these
//! calls. The controller keeps the drag state; the camera is a value, so each
//! call returns the camera the event leads to and the caller decides when to
//! show it.

use crate::authoring_binding::PyCamera;
use molgfx::controls::{
    ArcballController, Button, FlyController, InputEvent, Key, OrbitController,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn button(name: &str) -> PyResult<Button> {
    match name {
        "left" => Ok(Button::Left),
        "right" => Ok(Button::Right),
        "middle" => Ok(Button::Middle),
        _ => Err(PyValueError::new_err(
            "button must be 'left', 'right' or 'middle'",
        )),
    }
}

fn key(name: &str) -> PyResult<Key> {
    match name {
        "forward" => Ok(Key::Forward),
        "backward" => Ok(Key::Backward),
        "left" => Ok(Key::Left),
        "right" => Ok(Key::Right),
        "up" => Ok(Key::Up),
        "down" => Ok(Key::Down),
        _ => Err(PyValueError::new_err(
            "key must be 'forward', 'backward', 'left', 'right', 'up' or 'down'",
        )),
    }
}

/// Declares one controller class whose events are the pointer, wheel and pinch.
macro_rules! controller {
    ($class:ident, $name:literal, $inner:ty, $about:literal) => {
        #[doc = $about]
        #[pyclass(name = $name, skip_from_py_object)]
        pub(super) struct $class($inner);

        #[pymethods]
        impl $class {
            #[new]
            fn new() -> Self {
                Self(<$inner>::default())
            }

            /// The pointer moved to `(x, y)` pixels from the top-left.
            fn pointer_move(&mut self, x: f32, y: f32, camera: &PyCamera) -> PyCamera {
                self.update(InputEvent::PointerMove { x, y }, camera)
            }

            /// A pointer button was pressed or released at `(x, y)`.
            fn pointer_button(
                &mut self,
                button_name: &str,
                pressed: bool,
                x: f32,
                y: f32,
                camera: &PyCamera,
            ) -> PyResult<PyCamera> {
                Ok(self.update(
                    InputEvent::PointerButton {
                        button: button(button_name)?,
                        pressed,
                        x,
                        y,
                    },
                    camera,
                ))
            }

            /// The wheel scrolled; positive moves toward the scene.
            fn scroll(&mut self, delta: f32, camera: &PyCamera) -> PyCamera {
                self.update(InputEvent::Scroll { delta }, camera)
            }

            /// A trackpad pinch; a scale above one zooms in.
            fn pinch(&mut self, scale: f32, camera: &PyCamera) -> PyCamera {
                self.update(InputEvent::Pinch { scale }, camera)
            }
        }

        impl $class {
            fn update(&mut self, event: InputEvent, camera: &PyCamera) -> PyCamera {
                let mut moved = camera.0;
                self.0.update(event, &mut moved);
                PyCamera(moved)
            }
        }
    };
}

controller!(
    PyArcball,
    "ArcballController",
    ArcballController,
    "Rotation about the target in any direction; the inspection default."
);
controller!(
    PyOrbit,
    "OrbitController",
    OrbitController,
    "Rotation about the target that keeps the horizon level."
);
controller!(
    PyFly,
    "FlyController",
    FlyController,
    "First-person flight: drag to look, keys to move."
);

#[pymethods]
impl PyFly {
    /// A navigation key was pressed or released.
    fn key(&mut self, name: &str, pressed: bool, camera: &PyCamera) -> PyResult<PyCamera> {
        Ok(self.update(
            InputEvent::Key {
                key: key(name)?,
                pressed,
            },
            camera,
        ))
    }

    /// Moves the eye for the keys held over `seconds`, at `speed` Å per second.
    fn advance(&self, camera: &PyCamera, seconds: f32, speed: f32) -> PyCamera {
        let mut moved = camera.0;
        self.0.advance(&mut moved, seconds, speed);
        PyCamera(moved)
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyArcball>()?;
    module.add_class::<PyOrbit>()?;
    module.add_class::<PyFly>()
}

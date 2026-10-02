//! Cameras, the world-to-screen mapping and the camera controllers.
//!
//! A page translates its own pointer, wheel and key events into these calls and
//! draws what it likes over the canvas; none of the motion or projection
//! arithmetic lives in JavaScript. Every call returns a new camera: the host
//! decides when to show it.

use crate::contract::{javascript_error, vector3};
use molgfx::controls::{
    ArcballController, Button, FlyController, InputEvent, Key, OrbitController,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = Camera)]
#[derive(Clone, Copy, Debug)]
/// A validated look-at camera with a perspective projection.
pub struct WebCamera {
    pub(crate) inner: molgfx::Camera,
}

#[wasm_bindgen(js_class = Camera)]
impl WebCamera {
    /// Builds a camera from position, target and up vectors of three values.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for non-finite or degenerate input.
    #[wasm_bindgen(constructor)]
    pub fn new(
        position: &js_sys::Float32Array,
        target: &js_sys::Float32Array,
        up: &js_sys::Float32Array,
        fov_y: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Result<WebCamera, JsError> {
        molgfx::camera::perspective(
            vector3(position, "position")?,
            vector3(target, "target")?,
            vector3(up, "up")?,
            fov_y,
            aspect,
            near,
            far,
        )
        .map(|inner| Self { inner })
        .map_err(javascript_error)
    }

    /// The eye position.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn position(&self) -> Vec<f32> {
        self.inner.eye.to_array().to_vec()
    }

    /// The point looked at.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn target(&self) -> Vec<f32> {
        self.inner.target.to_array().to_vec()
    }

    /// The up hint.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn up(&self) -> Vec<f32> {
        self.inner.up.to_array().to_vec()
    }

    /// Where a world point lands on a render target: pixels from the top-left
    /// and the distance from the eye. Absent behind the eye.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when `point` does not have three values.
    pub fn project(
        &self,
        point: &js_sys::Float32Array,
        width: u32,
        height: u32,
    ) -> Result<Option<Vec<f32>>, JsError> {
        let point = vector3(point, "point")?;
        Ok(molgfx::camera::project(&self.inner, point, (width, height))
            .map(|screen| vec![screen.x, screen.y, screen.depth]))
    }

    /// The ray through a pixel: origin then unit direction, six values.
    #[must_use]
    pub fn ray(&self, x: f32, y: f32, width: u32, height: u32) -> Option<Vec<f32>> {
        molgfx::camera::ray(&self.inner, x, y, (width, height)).map(|ray| {
            let mut values = ray.origin.to_array().to_vec();
            values.extend(ray.direction.to_array());
            values
        })
    }
}

fn button(name: &str) -> Result<Button, JsError> {
    match name {
        "left" => Ok(Button::Left),
        "right" => Ok(Button::Right),
        "middle" => Ok(Button::Middle),
        _ => Err(JsError::new("button must be left, right or middle")),
    }
}

fn key(name: &str) -> Result<Key, JsError> {
    match name {
        "forward" => Ok(Key::Forward),
        "backward" => Ok(Key::Backward),
        "left" => Ok(Key::Left),
        "right" => Ok(Key::Right),
        "up" => Ok(Key::Up),
        "down" => Ok(Key::Down),
        _ => Err(JsError::new(
            "key must be forward, backward, left, right, up or down",
        )),
    }
}

/// Declares one controller class: pointer, wheel and pinch events.
macro_rules! controller {
    ($class:ident, $name:literal, $inner:ty) => {
        #[wasm_bindgen(js_name = $name)]
        #[derive(Debug, Default)]
        /// A camera controller driven by abstract input events.
        pub struct $class {
            inner: $inner,
        }

        #[wasm_bindgen(js_class = $name)]
        impl $class {
            /// Creates a controller with nothing held.
            #[wasm_bindgen(constructor)]
            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }

            /// The pointer moved to `(x, y)` pixels from the top-left.
            #[wasm_bindgen(js_name = pointerMove)]
            pub fn pointer_move(&mut self, x: f32, y: f32, camera: &WebCamera) -> WebCamera {
                self.update(InputEvent::PointerMove { x, y }, camera)
            }

            /// A pointer button was pressed or released at `(x, y)`.
            ///
            /// # Errors
            ///
            /// Returns a JavaScript error for an unknown button name.
            #[wasm_bindgen(js_name = pointerButton)]
            pub fn pointer_button(
                &mut self,
                button_name: &str,
                pressed: bool,
                x: f32,
                y: f32,
                camera: &WebCamera,
            ) -> Result<WebCamera, JsError> {
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
            pub fn scroll(&mut self, delta: f32, camera: &WebCamera) -> WebCamera {
                self.update(InputEvent::Scroll { delta }, camera)
            }

            /// A trackpad pinch; a scale above one zooms in.
            pub fn pinch(&mut self, scale: f32, camera: &WebCamera) -> WebCamera {
                self.update(InputEvent::Pinch { scale }, camera)
            }
        }

        impl $class {
            fn update(&mut self, event: InputEvent, camera: &WebCamera) -> WebCamera {
                let mut moved = camera.inner;
                self.inner.update(event, &mut moved);
                WebCamera { inner: moved }
            }
        }
    };
}

controller!(WebArcball, "ArcballController", ArcballController);
controller!(WebOrbit, "OrbitController", OrbitController);
controller!(WebFly, "FlyController", FlyController);

#[wasm_bindgen(js_class = FlyController)]
impl WebFly {
    /// A navigation key was pressed or released.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for an unknown key name.
    pub fn key(
        &mut self,
        name: &str,
        pressed: bool,
        camera: &WebCamera,
    ) -> Result<WebCamera, JsError> {
        Ok(self.update(
            InputEvent::Key {
                key: key(name)?,
                pressed,
            },
            camera,
        ))
    }

    /// Moves the eye for the keys held over `seconds`, at `speed` Å per second.
    #[must_use]
    pub fn advance(&self, camera: &WebCamera, seconds: f32, speed: f32) -> WebCamera {
        let mut moved = camera.inner;
        self.inner.advance(&mut moved, seconds, speed);
        WebCamera { inner: moved }
    }
}

//! Thin abstract-input adapters over the shared Rust camera controllers.

use pdviewx::{ArcballController, Button, FlyController, InputEvent, Key, OrbitController};
use wasm_bindgen::prelude::*;

use crate::browser_camera::WebCamera;

/// Shared controller selected explicitly by the caller.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebCameraControllerKind {
    /// Free tumble around the focus target.
    Arcball,
    /// Rotation with a fixed world-up horizon.
    Orbit,
    /// First-person view and held-key motion.
    Fly,
}

/// Abstract pointer button; browser event translation remains caller-owned.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebPointerButton {
    /// Primary pointer button.
    Left,
    /// Secondary pointer button.
    Right,
    /// Middle pointer button.
    Middle,
}

/// Navigation key understood by the shared fly controller.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebNavigationKey {
    /// Move in the view direction.
    Forward,
    /// Move against the view direction.
    Backward,
    /// Strafe left.
    Left,
    /// Strafe right.
    Right,
    /// Rise along world up.
    Up,
    /// Descend along world up.
    Down,
}

#[derive(Clone, Copy, Debug)]
enum Controller {
    Arcball(ArcballController),
    Orbit(OrbitController),
    Fly(FlyController),
}

/// Stateful controller consuming only abstract input values.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebCameraController {
    inner: Controller,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebCameraController {
    /// Creates the free-tumble inspection controller.
    pub fn arcball() -> Self {
        Self {
            inner: Controller::Arcball(ArcballController::default()),
        }
    }

    /// Creates the horizon-preserving orbit controller.
    pub fn orbit() -> Self {
        Self {
            inner: Controller::Orbit(OrbitController::default()),
        }
    }

    /// Creates the first-person flight controller.
    pub fn fly() -> Self {
        Self {
            inner: Controller::Fly(FlyController::default()),
        }
    }

    /// Active controller kind.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> WebCameraControllerKind {
        match self.inner {
            Controller::Arcball(_) => WebCameraControllerKind::Arcball,
            Controller::Orbit(_) => WebCameraControllerKind::Orbit,
            Controller::Fly(_) => WebCameraControllerKind::Fly,
        }
    }

    /// Delivers an abstract pointer position in logical pixels.
    #[wasm_bindgen(js_name = pointerMove)]
    pub fn pointer_move(&mut self, camera: &mut WebCamera, x: f32, y: f32) {
        self.update(InputEvent::PointerMove { x, y }, camera);
    }

    /// Delivers an abstract pointer-button transition.
    #[wasm_bindgen(js_name = pointerButton)]
    pub fn pointer_button(
        &mut self,
        camera: &mut WebCamera,
        button: WebPointerButton,
        pressed: bool,
        x: f32,
        y: f32,
    ) {
        self.update(
            InputEvent::PointerButton {
                button: button.into(),
                pressed,
                x,
                y,
            },
            camera,
        );
    }

    /// Delivers an abstract normalized scroll delta.
    pub fn scroll(&mut self, camera: &mut WebCamera, delta: f32) {
        self.update(InputEvent::Scroll { delta }, camera);
    }

    /// Delivers an abstract relative pinch scale.
    pub fn pinch(&mut self, camera: &mut WebCamera, scale: f32) {
        self.update(InputEvent::Pinch { scale }, camera);
    }

    /// Delivers an abstract navigation-key transition.
    pub fn key(&mut self, camera: &mut WebCamera, key: WebNavigationKey, pressed: bool) {
        self.update(
            InputEvent::Key {
                key: key.into(),
                pressed,
            },
            camera,
        );
    }

    /// Advances held-key flight motion in seconds and Ångström per second.
    ///
    /// # Errors
    ///
    /// Returns `TypeError` when this is not a fly controller.
    #[wasm_bindgen(js_name = flyAdvance)]
    pub fn fly_advance(
        &self,
        camera: &mut WebCamera,
        delta_seconds: f32,
        speed: f32,
    ) -> Result<(), JsValue> {
        let Controller::Fly(controller) = self.inner else {
            return Err(js_sys::TypeError::new("flyAdvance requires a fly controller").into());
        };
        controller.advance(&mut camera.inner, delta_seconds, speed);
        Ok(())
    }
}

impl WebCameraController {
    fn update(&mut self, event: InputEvent, camera: &mut WebCamera) {
        match &mut self.inner {
            Controller::Arcball(controller) => controller.update(event, &mut camera.inner),
            Controller::Orbit(controller) => controller.update(event, &mut camera.inner),
            Controller::Fly(controller) => controller.update(event, &mut camera.inner),
        }
    }
}

impl From<WebPointerButton> for Button {
    fn from(value: WebPointerButton) -> Self {
        match value {
            WebPointerButton::Left => Self::Left,
            WebPointerButton::Right => Self::Right,
            WebPointerButton::Middle => Self::Middle,
        }
    }
}

impl From<WebNavigationKey> for Key {
    fn from(value: WebNavigationKey) -> Self {
        match value {
            WebNavigationKey::Forward => Self::Forward,
            WebNavigationKey::Backward => Self::Backward,
            WebNavigationKey::Left => Self::Left,
            WebNavigationKey::Right => Self::Right,
            WebNavigationKey::Up => Self::Up,
            WebNavigationKey::Down => Self::Down,
        }
    }
}

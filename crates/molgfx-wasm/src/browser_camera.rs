//! Thin JavaScript camera value and abstract camera operations.

use molgfx::{
    core::{Button, InputEvent, OrbitController},
    math::{Camera, Projection, Vec3},
};
use wasm_bindgen::prelude::*;

use crate::browser::WebScene;

/// Browser camera without DOM listeners or browser event ownership.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebCamera {
    pub(crate) inner: Camera,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebCamera {
    /// Replaces the look-at pose in world coordinates.
    #[wasm_bindgen(js_name = setPose)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_pose(
        &mut self,
        eye_x: f32,
        eye_y: f32,
        eye_z: f32,
        target_x: f32,
        target_y: f32,
        target_z: f32,
        up_x: f32,
        up_y: f32,
        up_z: f32,
    ) {
        self.inner.eye = Vec3::new(eye_x, eye_y, eye_z);
        self.inner.target = Vec3::new(target_x, target_y, target_z);
        self.inner.up = Vec3::new(up_x, up_y, up_z).normalize_or_zero();
    }

    /// Uses a perspective projection with angles in radians and distances in Å.
    #[wasm_bindgen(js_name = setPerspective)]
    pub fn set_perspective(&mut self, fov_y: f32, aspect: f32, near: f32, far: f32) {
        self.inner.projection = Projection::Perspective {
            fov_y,
            aspect: aspect.max(1.0e-3),
            near: near.max(0.01),
            far: far.max(near + 0.01),
        };
    }

    /// Uses an orthographic projection with distances in Å.
    #[wasm_bindgen(js_name = setOrthographic)]
    pub fn set_orthographic(&mut self, height: f32, aspect: f32, near: f32, far: f32) {
        self.inner.projection = Projection::Orthographic {
            height: height.max(0.01),
            aspect: aspect.max(1.0e-3),
            near: near.max(0.01),
            far: far.max(near + 0.01),
        };
    }

    /// Updates only the viewport aspect while preserving projection and clipping.
    #[wasm_bindgen(js_name = setAspect)]
    pub fn set_aspect(&mut self, aspect: f32) {
        self.inner.projection.set_aspect(aspect.max(1.0e-3));
    }

    /// Refits near/far clipping to the current eye and complete scene bounds.
    #[wasm_bindgen(js_name = fitClipping)]
    pub fn fit_clipping(&mut self, scene: &WebScene) {
        self.inner
            .projection
            .fit_near_far(self.inner.eye, &scene.inner.world_aabb().bounding_sphere());
    }

    /// Applies one horizon-preserving orbit delta in logical pixels.
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        drag(&mut self.inner, Button::Left, delta_x, delta_y);
    }

    /// Applies one camera-plane pan delta in logical pixels.
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        drag(&mut self.inner, Button::Right, delta_x, delta_y);
    }

    /// Applies normalized scroll zoom; positive values move toward the target.
    pub fn zoom(&mut self, delta: f32) {
        OrbitController::default().update(InputEvent::Scroll { delta }, &mut self.inner);
    }

    /// Eye x coordinate.
    #[wasm_bindgen(getter, js_name = eyeX)]
    pub fn eye_x(&self) -> f32 {
        self.inner.eye.x
    }
    /// Eye y coordinate.
    #[wasm_bindgen(getter, js_name = eyeY)]
    pub fn eye_y(&self) -> f32 {
        self.inner.eye.y
    }
    /// Eye z coordinate.
    #[wasm_bindgen(getter, js_name = eyeZ)]
    pub fn eye_z(&self) -> f32 {
        self.inner.eye.z
    }
    /// Target x coordinate.
    #[wasm_bindgen(getter, js_name = targetX)]
    pub fn target_x(&self) -> f32 {
        self.inner.target.x
    }
    /// Target y coordinate.
    #[wasm_bindgen(getter, js_name = targetY)]
    pub fn target_y(&self) -> f32 {
        self.inner.target.y
    }
    /// Target z coordinate.
    #[wasm_bindgen(getter, js_name = targetZ)]
    pub fn target_z(&self) -> f32 {
        self.inner.target.z
    }
}

fn drag(camera: &mut Camera, button: Button, delta_x: f32, delta_y: f32) {
    let mut controller = OrbitController::default();
    controller.update(
        InputEvent::PointerButton {
            button,
            pressed: true,
            x: 0.0,
            y: 0.0,
        },
        camera,
    );
    controller.update(InputEvent::PointerMove { x: 0.0, y: 0.0 }, camera);
    controller.update(
        InputEvent::PointerMove {
            x: delta_x,
            y: delta_y,
        },
        camera,
    );
    controller.update(
        InputEvent::PointerButton {
            button,
            pressed: false,
            x: delta_x,
            y: delta_y,
        },
        camera,
    );
}

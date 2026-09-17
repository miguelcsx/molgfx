//! Scene-fit light-space transforms used by the raster shadow pass.

use super::LightingEnvironment;
use molgfx_core::Scene;
use molgfx_math::{Aabb, Camera, Mat4, Projection, Vec3};

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "shadow_tests.rs"]
mod tests;

/// The three matrices shared by shadow rasterization and HDR lighting.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ShadowMatrices {
    /// World-to-light-view transform.
    pub(crate) view: Mat4,
    /// Reversed-depth orthographic light projection.
    pub(crate) projection: Mat4,
    /// Light clip-from-world transform.
    pub(crate) view_projection: Mat4,
}

/// Revision-diffed world bound used by every shadow-producing render path.
#[derive(Debug)]
pub(crate) struct ShadowBoundCache {
    scene_identity: Option<u64>,
    bound: Aabb,
}

impl Default for ShadowBoundCache {
    fn default() -> Self {
        Self {
            scene_identity: None,
            bound: Aabb::EMPTY,
        }
    }
}

impl ShadowBoundCache {
    /// Recomputes heterogeneous scene bounds only after scene synchronization
    /// reports a change. Camera and lighting remain free to vary every frame.
    pub(crate) fn fit(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        lighting: LightingEnvironment,
        scene_changed: bool,
    ) -> ShadowMatrices {
        let identity = scene.cache_identity();
        if scene_changed || self.scene_identity != Some(identity) {
            self.bound = scene.world_aabb();
            self.scene_identity = Some(identity);
        }
        fit_bound(self.bound, camera, lighting)
    }

    #[cfg(test)]
    pub(super) const fn bound(&self) -> Aabb {
        self.bound
    }
}

fn fit_bound(bound: Aabb, camera: &Camera, lighting: LightingEnvironment) -> ShadowMatrices {
    let bound = if bound.is_empty() {
        Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0))
    } else {
        bound
    };
    let center = bound.center();
    let radius = bound.bounding_sphere().radius.max(1.0);
    let world_from_view = camera.view().inverse();
    let toward_light = normalized_or(
        world_from_view.transform_vector3(lighting.key_direction),
        Vec3::new(-0.42, 0.58, 0.70),
    );
    let eye = center + toward_light * (radius * 2.5 + 1.0);
    let up_hint = if toward_light.dot(Vec3::Y).abs() > 0.92 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = Camera::look_at(eye, center, up_hint);
    let light_bound = bound.transform(&view);
    let padding = radius * 0.08 + 0.25;
    let left = light_bound.min.x - padding;
    let right = light_bound.max.x + padding;
    let bottom = light_bound.min.y - padding;
    let top = light_bound.max.y + padding;
    let near = (-light_bound.max.z - padding).max(0.01);
    let far = (-light_bound.min.z + padding).max(near + 1.0);
    let projection = Projection::Orthographic {
        height: top - bottom,
        aspect: (right - left) / (top - bottom).max(1.0e-4),
        near,
        far,
    }
    .matrix();
    ShadowMatrices {
        view,
        projection,
        view_projection: projection * view,
    }
}

fn normalized_or(value: Vec3, fallback: Vec3) -> Vec3 {
    if value.is_finite() && value.length_squared() > 1.0e-8 {
        value.normalize()
    } else {
        fallback.normalize()
    }
}

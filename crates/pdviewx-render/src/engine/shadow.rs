//! Scene-fit light-space transforms used by the raster shadow pass.

use super::LightingEnvironment;
use pdviewx_core::Scene;
use pdviewx_math::{Aabb, Camera, Mat4, Projection, Vec3};

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

/// Fits a directional light to the current scene bound in `O(1)` after the
/// scene has supplied its cached world-space bound.
#[must_use]
pub(crate) fn fit(scene: &Scene, camera: &Camera, lighting: LightingEnvironment) -> ShadowMatrices {
    fit_bound(scene.world_aabb(), camera, lighting)
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

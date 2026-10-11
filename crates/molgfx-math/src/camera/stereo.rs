//! Parallel eye cameras with a shared zero-parallax plane; no toe-in distortion.
use crate::{Camera, Projection};

impl Camera {
    /// Derives left/right views with separation and convergence in world units.
    /// A zero separation gives two identical views. Orthographic views share
    /// the central camera's parallel projection, so they carry no false disparity.
    ///
    /// Returns `None` for invalid parameters or a degenerate camera.
    #[must_use]
    pub fn stereo_eyes(self, separation: f32, convergence: f32) -> Option<[Self; 2]> {
        if !separation.is_finite()
            || separation < 0.0
            || !convergence.is_finite()
            || convergence <= 0.0
        {
            return None;
        }
        let direction = (self.target - self.eye).try_normalize()?;
        let right = direction.cross(self.up).try_normalize()?;
        if !valid_projection(self.projection) {
            return None;
        }
        let mut cameras = [self; 2];
        for (camera, offset) in cameras
            .iter_mut()
            .zip([-separation * 0.5, separation * 0.5])
        {
            match self.projection {
                Projection::Perspective {
                    fov_y,
                    aspect,
                    near,
                    far,
                }
                | Projection::OffAxis {
                    fov_y,
                    aspect,
                    near,
                    far,
                    ..
                } => {
                    let existing_shift = match self.projection {
                        Projection::OffAxis { shift, .. } => shift,
                        _ => 0.0,
                    };
                    camera.eye += right * offset;
                    camera.target += right * offset;
                    camera.projection = Projection::OffAxis {
                        fov_y,
                        aspect,
                        near,
                        far,
                        shift: existing_shift
                            - offset / (convergence * (fov_y * 0.5).tan() * aspect),
                    };
                }
                Projection::Orthographic { .. } => {}
            }
        }
        cameras
            .iter()
            .all(|camera| {
                camera.eye.is_finite()
                    && camera.target.is_finite()
                    && camera.projection.matrix().is_finite()
            })
            .then_some(cameras)
    }
}

fn valid_projection(projection: Projection) -> bool {
    match projection {
        Projection::Perspective {
            fov_y,
            aspect,
            near,
            far,
        }
        | Projection::OffAxis {
            fov_y,
            aspect,
            near,
            far,
            ..
        } => {
            fov_y.is_finite()
                && fov_y > 0.0
                && fov_y < std::f32::consts::PI
                && aspect.is_finite()
                && aspect > 0.0
                && near.is_finite()
                && near > 0.0
                && far.is_finite()
                && far > near
                && projection.matrix().is_finite()
        }
        Projection::Orthographic {
            height,
            aspect,
            near,
            far,
        } => {
            height.is_finite()
                && height > 0.0
                && aspect.is_finite()
                && aspect > 0.0
                && near.is_finite()
                && near >= 0.0
                && far.is_finite()
                && far > near
        }
    }
}

#[cfg(test)]
#[path = "stereo_tests.rs"]
mod tests;

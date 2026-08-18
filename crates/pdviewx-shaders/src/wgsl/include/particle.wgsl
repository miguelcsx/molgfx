// Analytic and bounded implicit hits for caller-authored particle glyphs.
//
// Contracts:
//   - PrimitiveVsOut.orientation is a normalized quaternion.
//   - incoming ray direction is normalized.
//   - quaternion rotation therefore preserves ray/normal length.
//
// Analytic shapes avoid redundant normalization by exploiting their exact
// surface radius. Only Gaussian and superquadric gradients require a true
// normalization.
//
// One shape family per file under include/particle/; this file is the
// dispatcher, resolved at pipeline creation rather than per fragment.

//!include "include/particle/types.wgsl"
//!include "include/particle/radial.wgsl"
//!include "include/particle/capsule.wgsl"
//!include "include/particle/superquadric.wgsl"

/// Resolves exactly one shape selected at pipeline creation.
///
/// PARTICLE_SHAPE_KIND is an override constant, not per-fragment data.
/// Shape-specific draws must contain only instances of that shape.
fn particle_hit(
    in: PrimitiveVsOut,
    origin: vec3f,
    direction: vec3f,
) -> PrimitiveHit {
    switch PARTICLE_SHAPE_KIND {
        case PARTICLE_SHAPE_SPHERE: {
            return particle_sphere_hit(
                in,
                origin,
                direction,
            );
        }

        case PARTICLE_SHAPE_CYLINDER: {
            return particle_cylinder_hit(
                in,
                origin,
                direction,
            );
        }

        case PARTICLE_SHAPE_SPHEROCYLINDER: {
            return particle_spherocylinder_hit(
                in,
                origin,
                direction,
            );
        }

        case PARTICLE_SHAPE_GAUSSIAN: {
            return particle_gaussian_hit(
                in,
                origin,
                direction,
            );
        }

        case PARTICLE_SHAPE_CIRCLE: {
            return particle_circle_hit(
                in,
                origin,
                direction,
            );
        }

        case PARTICLE_SHAPE_SQUARE: {
            return particle_square_hit(
                in,
                origin,
                direction,
            );
        }

        case PARTICLE_SHAPE_SUPERQUADRIC: {
            return particle_superquadric_hit(
                in,
                origin,
                direction,
            );
        }

        default: {
            return particle_miss();
        }
    }
}

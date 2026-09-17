//! CPU lowering of heterogeneous scene primitives into one GPU record shape.

use molgfx_core::{
    EntityId, EntityKind, ParticleBoundary, ParticleMotionGpu, ParticleShape, Primitive,
    PrimitiveGpu, Scene,
};
use molgfx_math::{Mat3, Mat4, Quat, Vec3};

#[cfg(test)]
#[path = "primitive_packing_tests.rs"]
mod tests;

pub(super) fn pack_primitive(
    primitive: Primitive,
    row: u32,
    pick_page: u32,
    model: Mat4,
) -> Result<Option<(PrimitiveGpu, ParticleMotionGpu)>, molgfx_core::EntityIdError> {
    let entity_id = EntityId::pack(EntityKind::Primitive, u64::from(row))?.0;
    Ok(match primitive {
        Primitive::Ellipsoid {
            value,
            color,
            opacity,
            ..
        } => pack_ellipsoid(value, color.to_f32(), opacity, entity_id, pick_page, model)
            .map(|record| (record, ParticleMotionGpu::default())),
        Primitive::Carbohydrate(value) => pack_oriented(
            OrientedPrimitive {
                center: value.center,
                orientation: value.orientation,
                size: value.size,
                color: value.color.to_f32(),
                primitive: 1,
                shape: value.shape.stable_code(),
                parameters: [0.0; 2],
            },
            entity_id,
            pick_page,
            model,
        )
        .map(|record| (record, ParticleMotionGpu::default())),
        Primitive::Planar {
            value,
            color,
            opacity,
            ..
        } => {
            let orientation = Quat::from_mat3(&Mat3::from_cols(
                value.tangent,
                value.bitangent,
                value.normal,
            ));
            pack_oriented(
                OrientedPrimitive {
                    center: value.center,
                    orientation,
                    size: Vec3::new(value.size[0], value.size[1], 0.04),
                    color: with_opacity(color.to_f32(), opacity),
                    primitive: 2,
                    shape: 0,
                    parameters: [0.0; 2],
                },
                entity_id,
                pick_page,
                model,
            )
            .map(|record| (record, ParticleMotionGpu::default()))
        }
        Primitive::Particle(value) => pack_particle(value, entity_id, pick_page, model),
    })
}

fn pack_particle(
    value: molgfx_core::Particle,
    entity_id: u32,
    pick_page: u32,
    model: Mat4,
) -> Option<(PrimitiveGpu, ParticleMotionGpu)> {
    let mut color = with_opacity(value.color.to_f32(), value.opacity);
    if value.shape == ParticleShape::Gaussian {
        color[3] = color[3].min(0.998);
    }
    let record = pack_oriented(
        OrientedPrimitive {
            center: value.center,
            orientation: value.orientation,
            size: value.size,
            color,
            primitive: match value.shape {
                ParticleShape::Box => 2,
                ParticleShape::Sphere
                | ParticleShape::Cylinder
                | ParticleShape::Spherocylinder
                | ParticleShape::Gaussian
                | ParticleShape::Circle
                | ParticleShape::Square
                | ParticleShape::Superquadric => 3,
            },
            shape: value.shape as u32,
            parameters: value.shape_parameters,
        },
        entity_id,
        pick_page,
        model,
    )?;
    let motion = value
        .motion
        .map_or_else(ParticleMotionGpu::default, |motion| {
            pack_motion(motion, model)
        });
    Some((record, motion))
}

fn pack_motion(value: molgfx_core::ParticleMotion, model: Mat4) -> ParticleMotionGpu {
    let linear = Mat3::from_mat4(model);
    let displacement = linear * value.velocity() * value.fixed_timestep();
    let bounds = value.bounds().transform(&model);
    if !displacement.is_finite() || bounds.is_empty() {
        return ParticleMotionGpu::default();
    }
    let inverse_span = (bounds.max - bounds.min).max(Vec3::splat(1.0e-6)).recip();
    let boundary = match value.boundary() {
        ParticleBoundary::Bounce => 0,
        ParticleBoundary::Wrap => 1,
    };
    ParticleMotionGpu {
        velocity_step: [
            displacement.x,
            displacement.y,
            displacement.z,
            inverse_span.x,
        ],
        minimum: [bounds.min.x, bounds.min.y, bounds.min.z, inverse_span.y],
        maximum: [bounds.max.x, bounds.max.y, bounds.max.z, inverse_span.z],
        metadata: [1, boundary, value.seed(), value.respawn_after_steps()],
    }
}

#[derive(Clone, Copy)]
struct OrientedPrimitive {
    center: Vec3,
    orientation: Quat,
    size: Vec3,
    color: [f32; 4],
    primitive: u32,
    shape: u32,
    parameters: [f32; 2],
}

fn pack_ellipsoid(
    value: molgfx_core::AnisotropicEllipsoid,
    color: [f32; 4],
    opacity: f32,
    entity_id: u32,
    pick_page: u32,
    model: Mat4,
) -> Option<PrimitiveGpu> {
    let local_inverse = symmetric_matrix(value.inverse_tensor()?);
    let linear = Mat3::from_mat4(model);
    if !linear.is_finite() || linear.determinant().abs() <= 1.0e-6 {
        return None;
    }
    let inverse_linear = linear.inverse();
    let world_inverse = inverse_linear.transpose() * local_inverse * inverse_linear;
    let center = model.transform_point3(value.center());
    let bound = value.bounds().transform(&model).bounding_sphere().radius;
    Some(PrimitiveGpu {
        center_radius: [center.x, center.y, center.z, bound.max(1.0e-3)],
        orientation: [0.0, 0.0, 0.0, 1.0],
        size_opacity: [1.0, 1.0, 1.0, 1.0],
        inverse_primary: [
            world_inverse.x_axis.x,
            world_inverse.y_axis.y,
            world_inverse.z_axis.z,
            world_inverse.y_axis.x,
        ],
        inverse_cross: [world_inverse.z_axis.x, world_inverse.z_axis.y, 0.0, 0.0],
        color: with_opacity(color, opacity),
        metadata: [entity_id, pick_page, 0, 0],
    })
}

fn pack_oriented(
    primitive: OrientedPrimitive,
    entity_id: u32,
    pick_page: u32,
    model: Mat4,
) -> Option<PrimitiveGpu> {
    let linear = Mat3::from_mat4(model);
    let local = Mat3::from_quat(primitive.orientation);
    let transformed = linear * local;
    let (axis_x, axis_y, axis_z) = (transformed.x_axis, transformed.y_axis, transformed.z_axis);
    let scales = Vec3::new(axis_x.length(), axis_y.length(), axis_z.length());
    if !scales.is_finite() || scales.min_element() <= 1.0e-6 || !primitive.size.is_finite() {
        return None;
    }
    let x = axis_x / scales.x;
    let y = (axis_y - x * x.dot(axis_y)).try_normalize()?;
    let z = x.cross(y).try_normalize()?;
    let world_orientation = Quat::from_mat3(&Mat3::from_cols(x, y, z));
    let world_size = primitive.size * scales;
    let world_center = model.transform_point3(primitive.center);
    let bound = (world_size * 0.5).length();
    Some(PrimitiveGpu {
        center_radius: [
            world_center.x,
            world_center.y,
            world_center.z,
            bound.max(1.0e-3),
        ],
        orientation: world_orientation.to_array(),
        size_opacity: [world_size.x, world_size.y, world_size.z, 1.0],
        inverse_primary: [primitive.parameters[0], primitive.parameters[1], 0.0, 0.0],
        inverse_cross: [0.0; 4],
        color: primitive.color,
        metadata: [entity_id, pick_page, primitive.primitive, primitive.shape],
    })
}

fn symmetric_matrix(values: [f32; 6]) -> Mat3 {
    let [xx, yy, zz, xy, xz, yz] = values;
    Mat3::from_cols(
        Vec3::new(xx, xy, xz),
        Vec3::new(xy, yy, yz),
        Vec3::new(xz, yz, zz),
    )
}

fn with_opacity(mut color: [f32; 4], opacity: f32) -> [f32; 4] {
    color[3] *= opacity.clamp(0.0, 1.0);
    color
}

pub(super) fn placement_revision(scene: &Scene) -> u64 {
    let mut hash = 14_695_981_039_346_656_037u64;
    for (handle, placed) in scene.structures() {
        hash ^= u64::from(handle.row());
        hash = hash.wrapping_mul(1_099_511_628_211);
        for value in placed.model_to_world.to_cols_array() {
            hash ^= u64::from(value.to_bits());
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
    }
    hash
}

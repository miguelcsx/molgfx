//! Manifest conversion for caller-authored analytic primitives.

use super::super::types::{ParticleMotionDescription, PrimitiveDescription};
use super::{identity, rgba};
use crate::handle::RawHandle;
use crate::{ParticleBoundary, ParticleShape, Primitive};
use pdviewx_math::{Quat, Rgba8};

pub(crate) fn primitive_description(raw: RawHandle, value: &Primitive) -> PrimitiveDescription {
    let identity = identity(value.owner());
    let fields = primitive_fields(value);
    PrimitiveDescription {
        row: raw.row(),
        generation: raw.generation(),
        owner: identity,
        kind: fields.kind.to_owned(),
        visible: fields.visible,
        color: rgba(fields.color),
        opacity: fields.opacity,
        center: fields.center,
        orientation: fields.orientation,
        size: fields.size,
        tensor: fields.tensor,
        plane_axes: fields.plane_axes,
        shape: fields.shape.map(str::to_owned),
        shape_parameters: fields.shape_parameters,
        motion: fields.motion,
    }
}

struct PrimitiveFields {
    kind: &'static str,
    visible: bool,
    color: Rgba8,
    opacity: f32,
    center: [f32; 3],
    orientation: [f32; 4],
    size: [f32; 3],
    tensor: Option<[f32; 6]>,
    plane_axes: Option<[[f32; 3]; 3]>,
    shape: Option<&'static str>,
    shape_parameters: [f32; 2],
    motion: Option<ParticleMotionDescription>,
}

fn primitive_fields(value: &Primitive) -> PrimitiveFields {
    match *value {
        Primitive::Ellipsoid {
            value,
            color,
            opacity,
            visible,
            ..
        } => PrimitiveFields {
            kind: "ellipsoid",
            visible,
            color,
            opacity,
            center: value.center().to_array(),
            orientation: Quat::IDENTITY.to_array(),
            size: [0.0; 3],
            tensor: Some(value.tensor()),
            plane_axes: None,
            shape: None,
            shape_parameters: [1.0; 2],
            motion: None,
        },
        Primitive::Carbohydrate(value) => PrimitiveFields {
            kind: "carbohydrate",
            visible: value.visible,
            color: value.color,
            opacity: 1.0,
            center: value.center.to_array(),
            orientation: value.orientation.to_array(),
            size: value.size.to_array(),
            tensor: None,
            plane_axes: None,
            shape: Some(value.shape.stable_name()),
            shape_parameters: [1.0; 2],
            motion: None,
        },
        Primitive::Planar {
            value,
            color,
            opacity,
            visible,
        } => PrimitiveFields {
            kind: "planar",
            visible,
            color,
            opacity,
            center: value.center.to_array(),
            orientation: Quat::IDENTITY.to_array(),
            size: [value.size[0], value.size[1], 0.0],
            tensor: None,
            plane_axes: Some([
                value.normal.to_array(),
                value.tangent.to_array(),
                value.bitangent.to_array(),
            ]),
            shape: None,
            shape_parameters: [1.0; 2],
            motion: None,
        },
        Primitive::Particle(value) => PrimitiveFields {
            kind: "particle",
            visible: value.visible,
            color: value.color,
            opacity: value.opacity,
            center: value.center.to_array(),
            orientation: value.orientation.to_array(),
            size: value.size.to_array(),
            tensor: None,
            plane_axes: None,
            shape: Some(match value.shape {
                ParticleShape::Sphere => "sphere",
                ParticleShape::Box => "box",
                ParticleShape::Cylinder => "cylinder",
                ParticleShape::Spherocylinder => "spherocylinder",
                ParticleShape::Gaussian => "gaussian",
                ParticleShape::Circle => "circle",
                ParticleShape::Square => "square",
                ParticleShape::Superquadric => "superquadric",
            }),
            shape_parameters: value.shape_parameters,
            motion: value.motion.map(motion_description),
        },
    }
}

fn motion_description(value: crate::ParticleMotion) -> ParticleMotionDescription {
    ParticleMotionDescription {
        velocity: value.velocity().to_array(),
        bounds: [value.bounds().min.to_array(), value.bounds().max.to_array()],
        fixed_timestep: value.fixed_timestep(),
        seed: value.seed(),
        boundary: match value.boundary() {
            ParticleBoundary::Bounce => "bounce",
            ParticleBoundary::Wrap => "wrap",
        }
        .to_owned(),
        respawn_after_steps: value.respawn_after_steps(),
    }
}

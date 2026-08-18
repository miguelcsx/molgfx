//! One revision-diffed GPU table for analytic primitives.

use super::buffers::{count, upload_grow, write_draw_args};
use super::structure::GpuStructure;
use crate::error::RenderError;
use pdviewx_core::{
    EntityId, EntityKind, ParticleBoundary, ParticleMotionGpu, ParticleShape, Primitive,
    PrimitiveGpu, Scene,
};
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, Device};
use pdviewx_math::{Mat3, Mat4, Quat, Vec3};

#[derive(Debug)]
pub(super) struct GpuPrimitives<D: Device> {
    buffer: Option<D::Buffer>,
    previous: Option<D::Buffer>,
    motion: Option<D::Buffer>,
    args: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    motion_group: Option<D::BindGroup>,
    capacity: u64,
    previous_capacity: u64,
    motion_capacity: u64,
    count: u32,
    translucent: bool,
    motion_count: u32,
    synced: Option<(u64, u64, u64)>,
    scratch: Vec<PrimitiveGpu>,
    previous_scratch: Vec<[f32; 4]>,
    motion_scratch: Vec<ParticleMotionGpu>,
}

impl<D: Device> GpuPrimitives<D> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: None,
            previous: None,
            motion: None,
            args: None,
            group: None,
            motion_group: None,
            capacity: 0,
            previous_capacity: 0,
            motion_capacity: 0,
            count: 0,
            translucent: false,
            motion_count: 0,
            synced: None,
            scratch: Vec::new(),
            previous_scratch: Vec::new(),
            motion_scratch: Vec::new(),
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        motion_layout: &D::BindGroupLayout,
        scene: &Scene,
        structures: &[GpuStructure<D>],
    ) -> Result<bool, RenderError> {
        let revision = (
            scene.primitive_revision(),
            scene.structure_revision(),
            placement_revision(scene),
        );
        if self.synced == Some(revision) {
            return Ok(false);
        }
        self.scratch.clear();
        self.previous_scratch.clear();
        self.motion_scratch.clear();
        self.translucent = false;
        self.motion_count = 0;
        for (handle, primitive) in scene.primitives() {
            if !primitive.visible() {
                continue;
            }
            let owner = primitive.owner();
            let Some(placed) = scene.structure(owner) else {
                continue;
            };
            let Some(structure_id) = structures
                .iter()
                .find(|structure| structure.handle == owner)
                .map(GpuStructure::structure_id)
            else {
                continue;
            };
            let row = Scene::primitive_row(handle);
            if let Some((record, motion)) =
                pack_primitive(*primitive, row, structure_id, placed.model_to_world)
            {
                self.translucent |= record.color[3] < 0.999;
                self.motion_count += motion.metadata[0];
                self.previous_scratch.push(record.center_radius);
                self.scratch.push(record);
                self.motion_scratch.push(motion);
            }
        }
        let needed = (self.scratch.len() * std::mem::size_of::<PrimitiveGpu>()) as u64;
        let rebind = self.buffer.is_none()
            || self.previous.is_none()
            || self.motion.is_none()
            || needed > self.capacity;
        upload_grow(
            device,
            queue,
            "primitive records",
            &self.scratch,
            &mut self.buffer,
            &mut self.capacity,
        )?;
        upload_grow(
            device,
            queue,
            "primitive previous centers",
            &self.previous_scratch,
            &mut self.previous,
            &mut self.previous_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "primitive particle motion table",
            &self.motion_scratch,
            &mut self.motion,
            &mut self.motion_capacity,
        )?;
        write_draw_args(
            device,
            queue,
            "primitive indirect arguments",
            6,
            count(self.scratch.len()),
            &mut self.args,
        )?;
        if rebind || self.group.is_none() || self.motion_group.is_none() {
            self.bind(device, layout, motion_layout);
        }
        self.count = count(self.scratch.len());
        self.synced = Some(revision);
        Ok(true)
    }

    fn bind(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        motion_layout: &D::BindGroupLayout,
    ) {
        let (Some(buffer), Some(previous), Some(motion)) =
            (&self.buffer, &self.previous, &self.motion)
        else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: primitive table",
            layout,
            entries: &[
                BindGroupEntry::Buffer { binding: 0, buffer },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: previous,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: motion,
                },
            ],
        }));
        self.motion_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: primitive particle motion",
            layout: motion_layout,
            entries: &[
                BindGroupEntry::Buffer { binding: 0, buffer },
                BindGroupEntry::Buffer {
                    binding: 1,
                    buffer: previous,
                },
                BindGroupEntry::Buffer {
                    binding: 2,
                    buffer: motion,
                },
            ],
        }));
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.count > 0).then_some((self.group.as_ref()?, self.args.as_ref()?))
    }

    pub(super) fn transparent_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.translucent.then(|| self.draw()).flatten()
    }

    pub(super) fn particle_motion(&self) -> Option<(&D::BindGroup, u32)> {
        (self.motion_count > 0).then_some((self.motion_group.as_ref()?, self.count))
    }

    pub(super) const fn has_translucency(&self) -> bool {
        self.translucent
    }
}

fn pack_primitive(
    primitive: Primitive,
    row: u32,
    structure_id: u32,
    model: Mat4,
) -> Option<(PrimitiveGpu, ParticleMotionGpu)> {
    let entity_id = EntityId::pack(EntityKind::Primitive, row).0;
    match primitive {
        Primitive::Ellipsoid {
            value,
            color,
            opacity,
            ..
        } => pack_ellipsoid(
            value,
            color.to_f32(),
            opacity,
            entity_id,
            structure_id,
            model,
        )
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
            structure_id,
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
                structure_id,
                model,
            )
            .map(|record| (record, ParticleMotionGpu::default()))
        }
        Primitive::Particle(value) => {
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
                structure_id,
                model,
            )?;
            let motion = value
                .motion
                .map_or_else(ParticleMotionGpu::default, |motion| {
                    pack_motion(motion, model)
                });
            Some((record, motion))
        }
    }
}

fn pack_motion(value: pdviewx_core::ParticleMotion, model: Mat4) -> ParticleMotionGpu {
    let linear = Mat3::from_mat4(model);
    let displacement = linear * value.velocity() * value.fixed_timestep();
    let bounds = value.bounds().transform(&model);
    if !displacement.is_finite() || bounds.is_empty() {
        return ParticleMotionGpu::default();
    }
    let boundary = match value.boundary() {
        ParticleBoundary::Bounce => 0,
        ParticleBoundary::Wrap => 1,
    };
    ParticleMotionGpu {
        velocity_step: [displacement.x, displacement.y, displacement.z, 0.0],
        minimum: [bounds.min.x, bounds.min.y, bounds.min.z, 0.0],
        maximum: [bounds.max.x, bounds.max.y, bounds.max.z, 0.0],
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
    value: pdviewx_core::AnisotropicEllipsoid,
    color: [f32; 4],
    opacity: f32,
    entity_id: u32,
    structure_id: u32,
    model: Mat4,
) -> Option<PrimitiveGpu> {
    let local = value.inverse_tensor()?;
    let local_inverse = symmetric_matrix(local);
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
        metadata: [entity_id, structure_id, 0, 0],
    })
}

fn pack_oriented(
    primitive: OrientedPrimitive,
    entity_id: u32,
    structure_id: u32,
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
        metadata: [
            entity_id,
            structure_id,
            primitive.primitive,
            primitive.shape,
        ],
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

fn placement_revision(scene: &Scene) -> u64 {
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

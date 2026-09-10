//! Optional hardware acceleration for quality AO and shadows.
//!
//! The CPU BVHs remain resident and authoritative. Hardware acceleration is
//! an opportunistic traversal backend over the same analytic spheres and
//! capsules; any capability, allocation, build or device failure disables it
//! and leaves the compute path ready for the same frame.

use pdviewx_core::{AtomGpu, BondGpu, EntityKind, PlacedStructure};
use pdviewx_gpu::{
    AabbGeometry, AabbGeometrySize, AccelerationGeometryFlags, AccelerationStructureBinding,
    AccelerationStructureFlags, AccelerationStructureUpdateMode, BlasBuildDesc, BlasDesc,
    BlasGeometries, BlasGeometrySizes, BufferDesc, BufferUsage, CommandEncoder, Device, GpuError,
    Queue, RayQueryBindGroupDesc, TlasDesc, TlasInstance,
};
use pdviewx_math::{Aabb, Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HardwareFailure {
    Unavailable,
    Limit,
    DeviceLost,
    Backend,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RayAabb {
    lower: [f32; 3],
    upper: [f32; 3],
}

impl From<Aabb> for RayAabb {
    fn from(value: Aabb) -> Self {
        Self {
            lower: value.min.to_array(),
            upper: value.max.to_array(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingBuild {
    None,
    Tlas,
    BlasAndTlas,
}

#[derive(Debug)]
struct HardwareResources<D: Device> {
    aabbs: D::Buffer,
    aabb_capacity: u64,
    primitive_count: u32,
    size: AabbGeometrySize,
    blas: D::Blas,
    tlas: D::Tlas,
    group: D::BindGroup,
    pending: PendingBuild,
}

#[derive(Debug)]
pub(super) struct HardwareQuality<D: Device> {
    resources: Option<HardwareResources<D>>,
    scratch: Vec<RayAabb>,
    failure: Option<HardwareFailure>,
}

impl<D: Device> HardwareQuality<D> {
    pub(super) const fn new() -> Self {
        Self {
            resources: None,
            scratch: Vec::new(),
            failure: None,
        }
    }

    pub(super) fn group(&self) -> Option<&D::BindGroup> {
        self.resources.as_ref().map(|resources| &resources.group)
    }

    #[cfg(test)]
    pub(super) const fn failure(&self) -> Option<HardwareFailure> {
        self.failure
    }

    pub(super) fn sync_geometry(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: Option<&D::BindGroupLayout>,
        atoms: &[AtomGpu],
        bonds: &[BondGpu],
        placed: &PlacedStructure,
    ) {
        let Some(layout) = layout else {
            self.disable(HardwareFailure::Unavailable);
            return;
        };
        if !device.capabilities().ray_query() {
            self.disable(HardwareFailure::Unavailable);
            return;
        }
        self.scratch.clear();
        self.scratch.extend(
            atoms
                .iter()
                .map(|atom| RayAabb::from(atom_bound(atom, placed))),
        );
        self.scratch.extend(
            bonds
                .iter()
                .map(|bond| RayAabb::from(bond_bound(bond, atoms, placed))),
        );
        if self.scratch.is_empty() {
            self.disable(HardwareFailure::Unavailable);
            return;
        }
        if let Err(error) = self.rebuild(device, queue, layout, placed.model_to_world) {
            self.disable(classify(&error));
        }
    }

    pub(super) fn sync_placement(&mut self, device: &D, transform: Mat4) {
        let Some(resources) = &mut self.resources else {
            return;
        };
        let instance = TlasInstance {
            blas: &resources.blas,
            transform: affine_rows(transform),
            custom_data: 0,
            mask: u8::MAX,
        };
        if let Err(error) = device.set_tlas_instance(&mut resources.tlas, 0, Some(instance)) {
            self.disable(classify(&error));
            return;
        }
        resources.pending = match resources.pending {
            PendingBuild::BlasAndTlas => PendingBuild::BlasAndTlas,
            PendingBuild::None | PendingBuild::Tlas => PendingBuild::Tlas,
        };
    }

    pub(super) fn record(&mut self, encoder: &mut D::CommandEncoder) {
        let result = match &mut self.resources {
            Some(resources) => record_resources(encoder, resources),
            None => return,
        };
        if let Err(error) = result {
            self.disable(classify(&error));
        }
    }

    fn rebuild(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        transform: Mat4,
    ) -> Result<(), GpuError> {
        let primitive_count =
            u32::try_from(self.scratch.len()).map_err(|_| GpuError::LimitExceeded {
                resource: "quality ray-query primitives",
                limit: u64::from(u32::MAX),
            })?;
        let limits = device.ray_query_limits()?;
        if primitive_count > limits.max_blas_primitives
            || limits.max_blas_geometries == 0
            || limits.max_tlas_instances == 0
            || limits.max_bindings_per_shader_stage == 0
        {
            return Err(GpuError::LimitExceeded {
                resource: "quality ray-query acceleration",
                limit: u64::from(limits.max_blas_primitives),
            });
        }
        let bytes = bytemuck::cast_slice(self.scratch.as_slice());
        let needed = bytes.len() as u64;
        let recreate = self
            .resources
            .as_ref()
            .is_none_or(|resources| needed > resources.aabb_capacity);
        if recreate {
            self.resources = Some(create_resources(device, layout, primitive_count, needed)?);
        }
        let Some(resources) = &mut self.resources else {
            return Err(GpuError::DeviceLost);
        };
        resources.primitive_count = primitive_count;
        resources.size.primitive_count = primitive_count;
        queue.write_buffer(&resources.aabbs, 0, bytes);
        let instance = TlasInstance {
            blas: &resources.blas,
            transform: affine_rows(transform),
            custom_data: 0,
            mask: u8::MAX,
        };
        device.set_tlas_instance(&mut resources.tlas, 0, Some(instance))?;
        resources.pending = PendingBuild::BlasAndTlas;
        self.failure = None;
        Ok(())
    }

    fn disable(&mut self, failure: HardwareFailure) {
        self.resources = None;
        self.failure = Some(failure);
    }
}

fn create_resources<D: Device>(
    device: &D,
    layout: &D::BindGroupLayout,
    primitive_count: u32,
    needed: u64,
) -> Result<HardwareResources<D>, GpuError> {
    let capacity = match needed.checked_next_power_of_two() {
        Some(value) => value,
        None => needed,
    }
    .max(256);
    let aabbs = device.create_buffer(&BufferDesc {
        label: "quality analytic ray AABBs",
        size: capacity,
        usage: BufferUsage::COPY_DST.union(BufferUsage::BLAS_INPUT),
    })?;
    let size = AabbGeometrySize {
        primitive_count,
        flags: AccelerationGeometryFlags::empty(),
    };
    let blas = device.create_blas(&BlasDesc {
        label: "quality analytic BLAS",
        flags: AccelerationStructureFlags::ALLOW_UPDATE
            | AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: AccelerationStructureUpdateMode::PreferUpdate,
        geometries: BlasGeometrySizes::Aabbs(std::slice::from_ref(&size)),
    })?;
    let mut tlas = device.create_tlas(&TlasDesc {
        label: "quality placement TLAS",
        max_instances: 1,
        flags: AccelerationStructureFlags::ALLOW_UPDATE
            | AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: AccelerationStructureUpdateMode::PreferUpdate,
    })?;
    device.set_tlas_instance(
        &mut tlas,
        0,
        Some(TlasInstance {
            blas: &blas,
            transform: affine_rows(Mat4::IDENTITY),
            custom_data: 0,
            mask: u8::MAX,
        }),
    )?;
    let group = device.create_ray_query_bind_group(&RayQueryBindGroupDesc {
        label: "group3: quality ray-query scene",
        layout,
        entries: &[],
        acceleration_structures: &[AccelerationStructureBinding {
            binding: 0,
            tlas: &tlas,
        }],
    })?;
    Ok(HardwareResources {
        aabbs,
        aabb_capacity: capacity,
        primitive_count,
        size,
        blas,
        tlas,
        group,
        pending: PendingBuild::BlasAndTlas,
    })
}

fn record_resources<D: Device>(
    encoder: &mut D::CommandEncoder,
    resources: &mut HardwareResources<D>,
) -> Result<(), GpuError> {
    if resources.pending == PendingBuild::None {
        return Ok(());
    }
    if resources.pending == PendingBuild::BlasAndTlas {
        let geometry = AabbGeometry {
            size: &resources.size,
            buffer: &resources.aabbs,
            offset: 0,
            stride: std::mem::size_of::<RayAabb>() as u64,
        };
        encoder.build_blas(&BlasBuildDesc {
            blas: &resources.blas,
            geometries: BlasGeometries::Aabbs(std::slice::from_ref(&geometry)),
        })?;
    }
    encoder.build_tlas(&resources.tlas)?;
    resources.pending = PendingBuild::None;
    Ok(())
}

fn atom_bound(atom: &AtomGpu, placed: &PlacedStructure) -> Aabb {
    let mut bound = Aabb::EMPTY;
    let source = atom
        .entity_id
        .unpack()
        .and_then(|(kind, row)| (kind == EntityKind::Atom).then_some(row));
    if let Some(source) = source {
        extend_source(&mut bound, placed, source, atom.radius.abs());
    }
    bound
}

fn bond_bound(bond: &BondGpu, atoms: &[AtomGpu], placed: &PlacedStructure) -> Aabb {
    let mut box_bound = Aabb::EMPTY;
    for compact in [bond.atom_a, bond.atom_b] {
        let source = atoms
            .get(compact as usize)
            .and_then(|atom| atom.entity_id.unpack())
            .and_then(|(kind, row)| (kind == EntityKind::Atom).then_some(row));
        if let Some(source) = source {
            extend_source(&mut box_bound, placed, source, bond.radius.abs());
        }
    }
    box_bound
}

fn extend_source(bound: &mut Aabb, placed: &PlacedStructure, source: u32, radius: f32) {
    if let Some(segment) = placed.trajectory() {
        for positions in [segment.start().positions(), segment.end().positions()] {
            if let Some(position) = positions.get(source as usize) {
                bound.extend_sphere(Vec3::from_array(*position), radius);
            }
        }
    } else if let Some(position) = placed.atoms.coords().slice().get(source as usize) {
        bound.extend_sphere(Vec3::from_array(*position), radius);
    }
}

fn affine_rows(transform: Mat4) -> [f32; 12] {
    let x = transform.x_axis;
    let y = transform.y_axis;
    let z = transform.z_axis;
    let w = transform.w_axis;
    [x.x, y.x, z.x, w.x, x.y, y.y, z.y, w.y, x.z, y.z, z.z, w.z]
}

fn classify(error: &GpuError) -> HardwareFailure {
    match error {
        GpuError::Capability { .. } => HardwareFailure::Unavailable,
        GpuError::LimitExceeded { .. } => HardwareFailure::Limit,
        GpuError::DeviceLost => HardwareFailure::DeviceLost,
        _ => HardwareFailure::Backend,
    }
}

//! Shared hardware acceleration geometry for quality AO and shadows.
//!
//! The bottom-level structure covers the packed records and is therefore a
//! function of the record key, not of a placement: every representation that
//! draws the same records traverses the same BLAS. Only the top-level instance,
//! which carries the model transform, is per placement
//! ([`super::placement_acceleration`]).
//!
//! Hardware traversal is opportunistic. Any capability, allocation, build or
//! device failure disables it and leaves the compute path ready for the same
//! frame.

use molgfx_core::{AtomGpu, BondGpu, EntityKind, PlacedStructure};
use molgfx_gpu::{
    AabbGeometry, AabbGeometrySize, AccelerationGeometryFlags, AccelerationStructureFlags,
    AccelerationStructureUpdateMode, BlasBuildDesc, BlasDesc, BlasGeometries, BlasGeometrySizes,
    BufferDesc, BufferUsage, CommandEncoder, Device, GpuError, Queue,
};
use molgfx_math::{Aabb, Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HardwareFailure {
    Unavailable,
    Limit,
    DeviceLost,
    Backend,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct RayAabb {
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

#[derive(Debug)]
pub(super) struct QualityBlas<D: Device> {
    aabbs: Option<D::Buffer>,
    aabb_capacity: u64,
    primitive_count: u32,
    size: AabbGeometrySize,
    blas: Option<D::Blas>,
    pending: bool,
    failure: Option<HardwareFailure>,
}

impl<D: Device> QualityBlas<D> {
    pub(super) const fn new() -> Self {
        Self {
            aabbs: None,
            aabb_capacity: 0,
            primitive_count: 0,
            size: AabbGeometrySize {
                primitive_count: 0,
                flags: AccelerationGeometryFlags::empty(),
            },
            blas: None,
            pending: false,
            failure: None,
        }
    }

    pub(super) fn blas(&self) -> Option<&D::Blas> {
        self.blas.as_ref()
    }

    #[must_use]
    pub(super) const fn resident_bytes(&self) -> u64 {
        self.aabb_capacity
    }

    /// Rebuilds the AABB list and bottom-level structure for one record set.
    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        atoms: &[AtomGpu],
        bonds: &[BondGpu],
        placed: &PlacedStructure,
    ) {
        if !device.capabilities().ray_query() {
            self.disable(HardwareFailure::Unavailable);
            return;
        }
        let mut boxes = Vec::with_capacity(atoms.len().saturating_add(bonds.len()));
        boxes.extend(
            atoms
                .iter()
                .map(|atom| RayAabb::from(atom_bound(atom, placed))),
        );
        boxes.extend(
            bonds
                .iter()
                .map(|bond| RayAabb::from(bond_bound(bond, atoms, placed))),
        );
        if boxes.is_empty() {
            self.disable(HardwareFailure::Unavailable);
            return;
        }
        if let Err(error) = self.rebuild(device, queue, &boxes) {
            self.disable(classify(&error));
        }
    }

    pub(super) fn record(&mut self, encoder: &mut D::CommandEncoder) {
        if !self.pending {
            return;
        }
        let (Some(blas), Some(aabbs)) = (self.blas.as_ref(), self.aabbs.as_ref()) else {
            return;
        };
        let geometry = AabbGeometry {
            size: &self.size,
            buffer: aabbs,
            offset: 0,
            stride: std::mem::size_of::<RayAabb>() as u64,
        };
        match encoder.build_blas(&BlasBuildDesc {
            blas,
            geometries: BlasGeometries::Aabbs(std::slice::from_ref(&geometry)),
        }) {
            Ok(()) => self.pending = false,
            Err(error) => self.disable(classify(&error)),
        }
    }

    fn rebuild(
        &mut self,
        device: &D,
        queue: &D::Queue,
        bounds: &[RayAabb],
    ) -> Result<(), GpuError> {
        let primitive_count = u32::try_from(bounds.len()).map_err(|_| GpuError::LimitExceeded {
            resource: "quality ray-query primitives",
            limit: u64::from(u32::MAX),
        })?;
        let limits = device.ray_query_limits()?;
        if primitive_count > limits.max_blas_primitives || limits.max_blas_geometries == 0 {
            return Err(GpuError::LimitExceeded {
                resource: "quality ray-query acceleration",
                limit: u64::from(limits.max_blas_primitives),
            });
        }
        let bytes = bytemuck::cast_slice(bounds);
        let needed = bytes.len() as u64;
        if self.aabbs.is_none() || needed > self.aabb_capacity {
            // A byte count that cannot round up is already at the ceiling, so
            // the unrounded value is the only safe capacity.
            let capacity = match needed.checked_next_power_of_two() {
                Some(rounded) => rounded,
                None => needed,
            }
            .max(256);
            self.aabbs = Some(device.create_buffer(&BufferDesc {
                label: "quality analytic ray AABBs",
                size: capacity,
                usage: BufferUsage::COPY_DST.union(BufferUsage::BLAS_INPUT),
            })?);
            self.aabb_capacity = capacity;
            self.blas = Some(device.create_blas(&BlasDesc {
                label: "quality analytic BLAS",
                flags: AccelerationStructureFlags::ALLOW_UPDATE
                    | AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: AccelerationStructureUpdateMode::PreferUpdate,
                geometries: BlasGeometrySizes::Aabbs(std::slice::from_ref(&self.size)),
            })?);
        }
        let Some(aabbs) = self.aabbs.as_ref() else {
            return Err(GpuError::DeviceLost);
        };
        self.primitive_count = primitive_count;
        self.size.primitive_count = primitive_count;
        queue.write_buffer(aabbs, 0, bytes);
        self.pending = true;
        self.failure = None;
        Ok(())
    }

    fn disable(&mut self, failure: HardwareFailure) {
        self.aabbs = None;
        self.aabb_capacity = 0;
        self.blas = None;
        self.pending = false;
        self.failure = Some(failure);
    }
}

pub(super) fn classify(error: &GpuError) -> HardwareFailure {
    match error {
        GpuError::Capability { .. } => HardwareFailure::Unavailable,
        GpuError::LimitExceeded { .. } => HardwareFailure::Limit,
        GpuError::DeviceLost => HardwareFailure::DeviceLost,
        _ => HardwareFailure::Backend,
    }
}

pub(super) fn affine_rows(transform: Mat4) -> [f32; 12] {
    let x = transform.x_axis;
    let y = transform.y_axis;
    let z = transform.z_axis;
    let w = transform.w_axis;
    [x.x, y.x, z.x, w.x, x.y, y.y, z.y, w.y, x.z, y.z, z.z, w.z]
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

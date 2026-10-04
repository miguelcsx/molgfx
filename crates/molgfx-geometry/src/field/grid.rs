//! Borrowed grid access and affine boundary normals.

use super::FieldError;
use molgfx_core::{ScalarVolume, SegmentedVolume};
use molgfx_math::{Mat3, Mat4, Vec3};
use num_traits::ToPrimitive as _;

enum Values<'a> {
    Scalar(&'a [f32]),
    Labels(&'a [u32]),
}

pub(super) struct Grid<'a> {
    values: Values<'a>,
    pub(super) dimensions: [u16; 3],
    pub(super) transform: Mat4,
    normal_transform: Mat3,
}

#[derive(Clone, Copy)]
pub(super) struct Node {
    pub(super) identity: u64,
    pub(super) point: Vec3,
    pub(super) coordinates: [i32; 3],
}

impl<'a> Grid<'a> {
    pub(super) fn scalar(volume: &'a ScalarVolume) -> Result<Self, FieldError> {
        Self::new(
            Values::Scalar(volume.values()),
            volume.dimensions(),
            volume.voxel_to_world(),
        )
    }

    pub(super) fn labels(volume: &'a SegmentedVolume) -> Result<Self, FieldError> {
        Self::new(
            Values::Labels(volume.labels()),
            volume.dimensions(),
            volume.voxel_to_world(),
        )
    }

    fn new(values: Values<'a>, dimensions: [u32; 3], transform: Mat4) -> Result<Self, FieldError> {
        let mut narrow = [0; 3];
        for axis in 0..3 {
            narrow[axis] =
                u16::try_from(dimensions[axis]).map_err(|_| FieldError::IndexOverflow)?;
        }
        Ok(Self {
            values,
            dimensions: narrow,
            transform,
            normal_transform: Mat3::from_mat4(transform).inverse().transpose(),
        })
    }

    pub(super) fn cube(&self, origin: [i32; 3]) -> Result<[Node; 8], FieldError> {
        let mut nodes = [Node {
            identity: 0,
            point: Vec3::ZERO,
            coordinates: [0; 3],
        }; 8];
        let strides = self.dimensions.map(|dimension| u64::from(dimension) + 2);
        for (corner, node) in nodes.iter_mut().enumerate() {
            let mut point = [0.0; 3];
            let mut indices = [0; 3];
            for axis in 0..3 {
                let coordinate = origin[axis] + i32::from((corner & (1 << axis)) != 0);
                node.coordinates[axis] = coordinate;
                let magnitude = u16::try_from(coordinate.unsigned_abs())
                    .map_err(|_| FieldError::IndexOverflow)?;
                point[axis] = if coordinate < 0 {
                    -f32::from(magnitude)
                } else {
                    f32::from(magnitude)
                };
                indices[axis] =
                    u64::try_from(coordinate + 1).map_err(|_| FieldError::IndexOverflow)?;
            }
            node.point = Vec3::from_array(point);
            node.identity = indices[0] + strides[0] * (indices[1] + strides[1] * indices[2]);
        }
        Ok(nodes)
    }

    pub(super) fn refined_cube(&self, origin: [i32; 3]) -> Result<[Node; 8], FieldError> {
        let mut nodes = [Node {
            identity: 0,
            point: Vec3::ZERO,
            coordinates: [0; 3],
        }; 8];
        let strides = self
            .dimensions
            .map(|dimension| u64::from(dimension) * 2 + 3);
        for (corner, node) in nodes.iter_mut().enumerate() {
            let mut indices = [0; 3];
            let mut point = [0.0; 3];
            for axis in 0..3 {
                let coordinate = origin[axis] + i32::from(corner & (1 << axis) != 0);
                point[axis] = coordinate.to_f32().ok_or(FieldError::IndexOverflow)? * 0.5;
                indices[axis] =
                    u64::try_from(coordinate + 2).map_err(|_| FieldError::IndexOverflow)?;
            }
            node.point = Vec3::from_array(point);
            node.identity = indices[0] + strides[0] * (indices[1] + strides[1] * indices[2]);
        }
        Ok(nodes)
    }

    pub(super) fn label(&self, coordinates: [i32; 3]) -> Result<Option<u32>, FieldError> {
        let Values::Labels(labels) = self.values else {
            return Ok(None);
        };
        let Some(index) = self.index(coordinates)? else {
            return Ok(None);
        };
        Ok(Some(labels[index]))
    }

    pub(super) fn value(&self, coordinates: [i32; 3], label: u32) -> Result<f32, FieldError> {
        match self.values {
            Values::Labels(_) => {
                // The centre outweighs all neighbours, preserving every source
                // membership while rounding the reconstructed boundary.
                let mut weight = 8 * u8::from(self.label(coordinates)? == Some(label));
                for axis in 0..3 {
                    for offset in [-1, 1] {
                        let mut neighbour = coordinates;
                        neighbour[axis] += offset;
                        weight += u8::from(self.label(neighbour)? == Some(label));
                    }
                }
                Ok(f32::from(weight) / 14.0)
            }
            Values::Scalar(values) => {
                let clamped = std::array::from_fn(|axis| {
                    coordinates[axis].clamp(0, i32::from(self.dimensions[axis]) - 1)
                });
                let Some(index) = self.index(clamped)? else {
                    return Err(FieldError::IndexOverflow);
                };
                Ok(values[index])
            }
        }
    }

    fn index(&self, coordinates: [i32; 3]) -> Result<Option<usize>, FieldError> {
        if coordinates
            .iter()
            .zip(self.dimensions)
            .any(|(&coordinate, dimension)| coordinate < 0 || coordinate >= i32::from(dimension))
        {
            return Ok(None);
        }
        let mut offsets = [0; 3];
        for axis in 0..3 {
            offsets[axis] = usize::from(
                u16::try_from(coordinates[axis]).map_err(|_| FieldError::IndexOverflow)?,
            );
        }
        let dimensions = self.dimensions.map(usize::from);
        Ok(Some(
            offsets[0] + dimensions[0] * (offsets[1] + dimensions[1] * offsets[2]),
        ))
    }

    pub(super) fn sample(&self, point: Vec3, label: u32) -> Result<f32, FieldError> {
        let point = point.to_array();
        let floor = point.map(f32::floor);
        let mut lower = [0; 3];
        for axis in 0..3 {
            lower[axis] = floor[axis].to_i32().ok_or(FieldError::IndexOverflow)?;
        }
        // Exterior padding has negative coordinates; use the distance from
        // the lower node rather than a sign-preserving fractional part.
        let fraction: [f32; 3] = std::array::from_fn(|axis| point[axis] - floor[axis]);
        let mut value = 0.0;
        for corner in 0..8 {
            let mut coordinate = lower;
            let mut weight = 1.0;
            for axis in 0..3 {
                if corner & (1 << axis) == 0 {
                    weight *= 1.0 - fraction[axis];
                } else {
                    coordinate[axis] += 1;
                    weight *= fraction[axis];
                }
            }
            value += weight * self.value(coordinate, label)?;
        }
        Ok(value)
    }

    pub(super) fn normal(&self, point: Vec3, label: u32) -> Result<Vec3, FieldError> {
        let mut gradient = [0.0; 3];
        for (axis, component) in gradient.iter_mut().enumerate() {
            let mut low = point.to_array();
            let mut high = point.to_array();
            low[axis] -= 1.0;
            high[axis] += 1.0;
            if matches!(self.values, Values::Scalar(_)) {
                low[axis] = low[axis].max(0.0);
                high[axis] = high[axis].min(f32::from(self.dimensions[axis] - 1));
            }
            *component = (self.sample(Vec3::from_array(low), label)?
                - self.sample(Vec3::from_array(high), label)?)
                / (high[axis] - low[axis]);
        }
        let normal = self.normal_transform * Vec3::from_array(gradient);
        if !normal.is_finite() {
            return Err(FieldError::NonfiniteGeometry);
        }
        let length = normal.x.hypot(normal.y).hypot(normal.z);
        Ok(if length == 0.0 {
            Vec3::ZERO
        } else {
            normal / length
        })
    }
}

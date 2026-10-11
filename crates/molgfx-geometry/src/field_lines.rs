//! Bounded streamline integration over the physical gradient of an affine grid.

use molgfx_core::{ScalarVolume, VolumeRegion};
use molgfx_math::{Mat4, Vec3};
use num_traits::ToPrimitive as _;

#[cfg(test)]
#[path = "field_lines_tests.rs"]
mod tests;

/// Integration controls in world ångström and scalar units per ångström.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldLineParams {
    /// Seed spacing in voxels on each axis.
    pub seed_stride: u32,
    /// Inclusive seed gradient-magnitude interval.
    pub seed_gradient: [f32; 2],
    /// RK4 arclength step in world units.
    pub step: f32,
    /// Inclusive displayed arclength interval.
    pub length: [f32; 2],
    /// Gradient below which an integration branch stops.
    pub critical_gradient: f32,
    /// Maximum steps on each side of a seed.
    pub max_steps: u32,
    /// Hard output segment budget; exhaustion is an error, never truncation.
    pub max_segments: usize,
}

impl Default for FieldLineParams {
    fn default() -> Self {
        Self {
            seed_stride: 4,
            seed_gradient: [0.0, f32::MAX],
            step: 0.25,
            length: [1.0, 50.0],
            critical_gradient: 1.0e-6,
            max_steps: 1024,
            max_segments: 1_000_000,
        }
    }
}

/// Validation or explicit resource-limit failure during field integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldLineError {
    /// Controls must be finite, ordered and positive where required.
    InvalidParameters,
    /// The authored output exceeds the explicit segment budget.
    SegmentBudget,
    /// The seed grid exceeds the bounded integration workload.
    SeedBudget,
}

impl std::fmt::Display for FieldLineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidParameters => "field-line controls must be finite, ordered and bounded",
            Self::SegmentBudget => "field lines exceed their segment budget",
            Self::SeedBudget => "field lines exceed the 65536-seed budget",
        })
    }
}

impl std::error::Error for FieldLineError {}

impl FieldLineParams {
    /// Validates controls before any seed or integration storage is allocated.
    ///
    /// # Errors
    /// Rejects non-finite, unordered or unbounded controls.
    pub fn validate(self) -> Result<(), FieldLineError> {
        if self.seed_stride == 0
            || !(1..=4096).contains(&self.max_steps)
            || self.max_segments == 0
            || self.max_segments > 4_000_000
            || !self.step.is_finite()
            || self.step <= 0.0
            || self.length.iter().any(|value| !value.is_finite())
            || self.length[0] < 0.0
            || self.length[1] <= self.length[0]
            || self.seed_gradient.iter().any(|value| !value.is_finite())
            || self.seed_gradient[0] < 0.0
            || self.seed_gradient[1] < self.seed_gradient[0]
            || !self.critical_gradient.is_finite()
            || self.critical_gradient <= 0.0
        {
            return Err(FieldLineError::InvalidParameters);
        }
        Ok(())
    }
}

/// Integrates both directions from regularly spaced seeds and returns open paths.
/// Uses the inverse transpose for gradients, including skewed and rotated grids.
/// A critical point or the grid boundary terminates a branch; paths outside the
/// requested length interval are omitted. Input voxels remain borrowed.
///
/// # Errors
/// Returns explicit invalid-control, seed-budget or segment-budget errors.
pub fn field_lines(
    volume: &ScalarVolume,
    params: FieldLineParams,
) -> Result<Vec<Vec<Vec3>>, FieldLineError> {
    field_lines_in_region(volume, params, None)
}

/// Integrates only inside an optional validated voxel crop.
///
/// # Errors
/// Returns the same bounded-work errors as [`field_lines`].
pub fn field_lines_in_region(
    volume: &ScalarVolume,
    params: FieldLineParams,
    region: Option<VolumeRegion>,
) -> Result<Vec<Vec<Vec3>>, FieldLineError> {
    params.validate()?;
    let dimensions = volume.dimensions();
    let minimum = region.map_or([0; 3], VolumeRegion::minimum);
    let maximum = region.map_or(dimensions, VolumeRegion::maximum);
    if (0..3).any(|axis| maximum[axis] > dimensions[axis] || minimum[axis] >= maximum[axis]) {
        return Err(FieldLineError::InvalidParameters);
    }
    let extents = [
        maximum[0] - minimum[0],
        maximum[1] - minimum[1],
        maximum[2] - minimum[2],
    ];
    let seeds = extents
        .iter()
        .try_fold(1u64, |count, dimension| {
            count.checked_mul(u64::from(dimension.div_ceil(params.seed_stride)))
        })
        .ok_or(FieldLineError::SeedBudget)?;
    if seeds > 65_536 {
        return Err(FieldLineError::SeedBudget);
    }
    let field = GradientField {
        volume,
        world_to_voxel: volume.voxel_to_world().inverse(),
        minimum,
        maximum,
    };
    let stride =
        usize::try_from(params.seed_stride).map_err(|_| FieldLineError::InvalidParameters)?;
    let mut paths = Vec::new();
    let mut segments = 0usize;
    for z in (minimum[2]..maximum[2]).step_by(stride) {
        for y in (minimum[1]..maximum[1]).step_by(stride) {
            for x in (minimum[0]..maximum[0]).step_by(stride) {
                let seed = volume.voxel_to_world().transform_point3(Vec3::new(
                    number(x),
                    number(y),
                    number(z),
                ));
                let Some(gradient) = field.at(seed) else {
                    continue;
                };
                let magnitude = gradient.length();
                if magnitude < params.seed_gradient[0]
                    || magnitude > params.seed_gradient[1]
                    || magnitude < params.critical_gradient
                {
                    continue;
                }
                let (mut backward, left) =
                    branch(&field, seed, -1.0, params, params.length[1] * 0.5);
                let (forward, right) = branch(&field, seed, 1.0, params, params.length[1] - left);
                let length = left + right;
                if length < params.length[0] || backward.len() + forward.len() < 3 {
                    continue;
                }
                backward.reverse();
                backward.extend(forward.into_iter().skip(1));
                segments = segments
                    .checked_add(backward.len() - 1)
                    .ok_or(FieldLineError::SegmentBudget)?;
                if segments > params.max_segments {
                    return Err(FieldLineError::SegmentBudget);
                }
                paths.push(backward);
            }
        }
    }
    Ok(paths)
}

fn branch(
    field: &GradientField<'_>,
    seed: Vec3,
    sign: f32,
    params: FieldLineParams,
    limit: f32,
) -> (Vec<Vec3>, f32) {
    let mut points = vec![seed];
    let mut position = seed;
    let mut length = 0.0;
    for _ in 0..params.max_steps {
        let step = params.step.min(limit - length);
        if step <= 1.0e-6 {
            break;
        }
        let direction = |point| {
            field.at(point).and_then(|gradient| {
                (gradient.length() >= params.critical_gradient).then(|| gradient.normalize() * sign)
            })
        };
        let Some(a) = direction(position) else {
            break;
        };
        let Some(b) = direction(position + a * (step * 0.5)) else {
            break;
        };
        let Some(c) = direction(position + b * (step * 0.5)) else {
            break;
        };
        let Some(d) = direction(position + c * step) else {
            break;
        };
        let next = position + (a + b * 2.0 + c * 2.0 + d) * (step / 6.0);
        let distance = next.distance(position);
        if distance <= 1.0e-6 || field.at(next).is_none() {
            break;
        }
        length += distance;
        points.push(next);
        position = next;
    }
    (points, length)
}

struct GradientField<'a> {
    volume: &'a ScalarVolume,
    world_to_voxel: Mat4,
    minimum: [u32; 3],
    maximum: [u32; 3],
}

impl GradientField<'_> {
    fn at(&self, world: Vec3) -> Option<Vec3> {
        let position = self.world_to_voxel.transform_point3(world);
        let dimensions = self.volume.dimensions();
        let minimum = Vec3::new(
            number(self.minimum[0]),
            number(self.minimum[1]),
            number(self.minimum[2]),
        );
        let maximum = Vec3::new(
            number(self.maximum[0] - 1),
            number(self.maximum[1] - 1),
            number(self.maximum[2] - 1),
        );
        if !position.is_finite()
            || (position - minimum).min_element() < 0.0
            || (maximum - position).min_element() < 0.0
        {
            return None;
        }
        let base = [
            position.x.floor().to_u32()?,
            position.y.floor().to_u32()?,
            position.z.floor().to_u32()?,
        ];
        let mut gradient = Vec3::ZERO;
        for dz in 0..=1 {
            for dy in 0..=1 {
                for dx in 0..=1 {
                    let offset = [dx, dy, dz];
                    let mut weight = 1.0;
                    let mut index = [0; 3];
                    for axis in 0..3 {
                        let fraction = position.to_array()[axis] - number(base[axis]);
                        weight *= if offset[axis] == 0 {
                            1.0 - fraction
                        } else {
                            fraction
                        };
                        index[axis] = (base[axis] + offset[axis]).min(dimensions[axis] - 1);
                    }
                    let mut nodal = [0.0; 3];
                    for axis in 0..3 {
                        let mut low = index;
                        let mut high = index;
                        low[axis] = low[axis].saturating_sub(1);
                        high[axis] = (high[axis] + 1).min(dimensions[axis] - 1);
                        nodal[axis] =
                            (self.value(high)? - self.value(low)?) / number(high[axis] - low[axis]);
                    }
                    gradient += Vec3::from_array(nodal) * weight;
                }
            }
        }
        let inverse = self.world_to_voxel.to_cols_array();
        Some(Vec3::new(
            gradient.dot(Vec3::new(inverse[0], inverse[1], inverse[2])),
            gradient.dot(Vec3::new(inverse[4], inverse[5], inverse[6])),
            gradient.dot(Vec3::new(inverse[8], inverse[9], inverse[10])),
        ))
    }

    fn value(&self, index: [u32; 3]) -> Option<f32> {
        let dimensions = self.volume.dimensions();
        let row = (u64::from(index[2]) * u64::from(dimensions[1]) + u64::from(index[1]))
            * u64::from(dimensions[0])
            + u64::from(index[0]);
        self.volume
            .values()
            .get(usize::try_from(row).ok()?)
            .copied()
    }
}

fn number(value: u32) -> f32 {
    u16::try_from(value).map_or(f32::NAN, f32::from)
}

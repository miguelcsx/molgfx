//! Caller-supplied analytic primitives with deterministic geometry helpers.
//!
//! These types carry results from a parser, crystallographic library or
//! validation tool into the renderer. They do not inspect structures or infer
//! chemistry. Unit-cell edges can be lowered to the existing guide table;
//! ellipsoids and carbohydrate symbols expose the validated data consumed by
//! the renderer's shared analytic primitive table without inferring chemistry.

use crate::{CoreError, Particle, StructureHandle};
use molgfx_math::{Aabb, Mat3, Mat4, Quat, Vec3};

#[cfg(test)]
#[path = "primitive_tests.rs"]
mod tests;

const EPSILON: f32 = 1.0e-6;

/// One caller-declared symmetry or assembly transform for a unit cell.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SymmetryInstance {
    /// Stable caller-defined instance id.
    pub id: u32,
    /// Affine Cartesian transform.
    pub transform: Mat4,
}

impl SymmetryInstance {
    /// Creates a non-singular finite transform.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for a non-invertible
    /// or non-finite transform.
    pub fn new(id: u32, transform: Mat4) -> Result<Self, CoreError> {
        if !transform.is_finite() || transform.determinant().abs() <= EPSILON {
            return Err(invalid("symmetry transform must be finite and invertible"));
        }
        Ok(Self { id, transform })
    }
}

/// A positive-definite anisotropic displacement ellipsoid.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AnisotropicEllipsoid {
    center: Vec3,
    /// Symmetric tensor `[U11, U22, U33, U12, U13, U23]` in Å².
    tensor: [f32; 6],
}

impl AnisotropicEllipsoid {
    /// Creates one unit-level ellipsoid from a symmetric displacement tensor.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] unless the tensor is
    /// finite and positive definite.
    pub fn new(center: Vec3, tensor: [f32; 6]) -> Result<Self, CoreError> {
        if !center.is_finite() || tensor.iter().any(|value| !value.is_finite()) {
            return Err(invalid("ellipsoid center and tensor must be finite"));
        }
        let [u11, u22, u33, u12, u13, u23] = tensor;
        let leading_1 = u11;
        let leading_2 = u11 * u22 - u12 * u12;
        let determinant = u11.mul_add(u22 * u33 - u23 * u23, -u12 * (u12 * u33 - u13 * u23))
            + u13 * (u12 * u23 - u22 * u13);
        if leading_1 <= EPSILON || leading_2 <= EPSILON || determinant <= EPSILON {
            return Err(invalid("ellipsoid tensor must be positive definite"));
        }
        Ok(Self { center, tensor })
    }

    /// Center in the owning structure's model space.
    #[must_use]
    pub const fn center(self) -> Vec3 {
        self.center
    }

    /// Tensor in `[U11, U22, U33, U12, U13, U23]` order.
    #[must_use]
    pub const fn tensor(self) -> [f32; 6] {
        self.tensor
    }

    /// Inverse tensor in the same symmetric order, for analytic GPU hits.
    #[must_use]
    pub fn inverse_tensor(self) -> Option<[f32; 6]> {
        let inverse = inverse_symmetric(self.tensor)?;
        Some([
            inverse.x_axis.x,
            inverse.y_axis.y,
            inverse.z_axis.z,
            inverse.y_axis.x,
            inverse.z_axis.x,
            inverse.z_axis.y,
        ])
    }

    /// Conservative axis-aligned bounds used by culling and shadow fitting.
    #[must_use]
    pub fn bounds(self) -> Aabb {
        let extent = Vec3::new(self.tensor[0], self.tensor[1], self.tensor[2]).sqrt();
        Aabb::new(self.center - extent, self.center + extent)
    }

    /// Analytic ray interval for `xᵀ U⁻¹ x = 1`, if the ray hits.
    #[must_use]
    pub fn ray_intersection(self, origin: Vec3, direction: Vec3) -> Option<(f32, f32)> {
        let inverse = inverse_symmetric(self.tensor)?;
        if !origin.is_finite() || !direction.is_finite() || direction.length_squared() <= EPSILON {
            return None;
        }
        let offset = origin - self.center;
        let inv_direction = inverse * direction;
        let inv_offset = inverse * offset;
        let a = direction.dot(inv_direction);
        let b = 2.0 * offset.dot(inv_direction);
        let c = offset.dot(inv_offset) - 1.0;
        let discriminant = b.mul_add(b, -4.0 * a * c);
        if !discriminant.is_finite() || discriminant < 0.0 || a <= EPSILON {
            return None;
        }
        let root = discriminant.sqrt();
        let first = (-b - root) / (2.0 * a);
        let second = (-b + root) / (2.0 * a);
        Some((first.min(second), first.max(second)))
    }
}

fn inverse_symmetric([u11, u22, u33, u12, u13, u23]: [f32; 6]) -> Option<Mat3> {
    let determinant = u11.mul_add(u22 * u33 - u23 * u23, -u12 * (u12 * u33 - u13 * u23))
        + u13 * (u12 * u23 - u22 * u13);
    if !determinant.is_finite() || determinant.abs() <= EPSILON {
        return None;
    }
    let inv = 1.0 / determinant;
    Some(Mat3::from_cols(
        Vec3::new(
            (u22 * u33 - u23 * u23) * inv,
            (u13 * u23 - u12 * u33) * inv,
            (u12 * u23 - u22 * u13) * inv,
        ),
        Vec3::new(
            (u13 * u23 - u12 * u33) * inv,
            (u11 * u33 - u13 * u13) * inv,
            (u12 * u13 - u11 * u23) * inv,
        ),
        Vec3::new(
            (u12 * u23 - u22 * u13) * inv,
            (u12 * u13 - u11 * u23) * inv,
            (u11 * u22 - u12 * u12) * inv,
        ),
    ))
}

/// SNFG-style symbol families accepted from a carbohydrate-aware caller.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum CarbohydrateShape {
    /// Unclassified six-membered ring.
    #[default]
    Unknown,
    /// Glucose.
    Glc,
    /// Galactose.
    Gal,
    /// Mannose.
    Man,
    /// Fucose.
    Fuc,
    /// Xylose.
    Xyl,
    /// N-acetylneuraminic acid.
    Neu5Ac,
}

impl CarbohydrateShape {
    /// Stable label for manifests and glyph lookup tables.
    #[must_use]
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Glc => "glc",
            Self::Gal => "gal",
            Self::Man => "man",
            Self::Fuc => "fuc",
            Self::Xyl => "xyl",
            Self::Neu5Ac => "neu5ac",
        }
    }

    /// Stable numeric code used by the GPU glyph lookup table.
    #[must_use]
    pub const fn stable_code(self) -> u32 {
        match self {
            Self::Unknown => 0,
            Self::Glc => 1,
            Self::Gal => 2,
            Self::Man => 3,
            Self::Fuc => 4,
            Self::Xyl => 5,
            Self::Neu5Ac => 6,
        }
    }
}

/// One caller-resolved carbohydrate symbol.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CarbohydrateSymbol {
    /// Structure that owns the model-space position.
    pub owner: StructureHandle,
    /// Symbol center in model space.
    pub center: Vec3,
    /// Orientation of the symbol plane.
    pub orientation: Quat,
    /// Positive symbol dimensions in ångström.
    pub size: Vec3,
    /// Semantic symbol family.
    pub shape: CarbohydrateShape,
    /// Display colour selected by the caller.
    pub color: molgfx_math::Rgba8,
    /// Whether a future symbol pass should include this record.
    pub visible: bool,
}

impl CarbohydrateSymbol {
    /// Validates one symbol record without deriving residue chemistry.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for malformed pose or
    /// dimensions.
    pub fn new(
        owner: StructureHandle,
        center: Vec3,
        orientation: Quat,
        size: Vec3,
        shape: CarbohydrateShape,
        color: molgfx_math::Rgba8,
    ) -> Result<Self, CoreError> {
        if !center.is_finite()
            || !orientation.is_finite()
            || orientation.length_squared() <= EPSILON
            || !size.is_finite()
            || size.min_element() <= EPSILON
        {
            return Err(invalid(
                "carbohydrate pose and size must be finite and positive",
            ));
        }
        Ok(Self {
            owner,
            center,
            orientation: orientation.normalize(),
            size,
            shape,
            color,
            visible: true,
        })
    }

    /// Eight oriented-box corners, useful for conservative culling.
    #[must_use]
    pub fn bounds(self) -> Aabb {
        let half = self.size * 0.5;
        Aabb::from_points(
            [
                Vec3::new(-half.x, -half.y, -half.z),
                Vec3::new(half.x, -half.y, -half.z),
                Vec3::new(-half.x, half.y, -half.z),
                Vec3::new(half.x, half.y, -half.z),
                Vec3::new(-half.x, -half.y, half.z),
                Vec3::new(half.x, -half.y, half.z),
                Vec3::new(-half.x, half.y, half.z),
                Vec3::new(half.x, half.y, half.z),
            ]
            .map(|corner| self.center + self.orientation * corner),
        )
    }
}

/// A renderable caller-authored primitive.
///
/// The scene stores the owner and presentation beside the validated geometry
/// so one GPU table can draw heterogeneous primitives with one indirect call.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Primitive {
    /// An anisotropic displacement ellipsoid.
    Ellipsoid {
        /// Structure carrying the model-space center.
        owner: StructureHandle,
        /// Positive-definite displacement tensor.
        value: AnisotropicEllipsoid,
        /// Display color.
        color: molgfx_math::Rgba8,
        /// Final alpha.
        opacity: f32,
        /// Whether the record participates in the draw.
        visible: bool,
    },
    /// An SNFG-style caller-resolved carbohydrate symbol.
    Carbohydrate(CarbohydrateSymbol),
    /// A filled rectangular primitive plane.
    Planar {
        /// Validated plane pose and dimensions.
        value: crate::PlanarRegion,
        /// Display color.
        color: molgfx_math::Rgba8,
        /// Final alpha.
        opacity: f32,
        /// Whether the record participates in the draw.
        visible: bool,
    },
    /// A generic caller-authored particle glyph.
    Particle(Particle),
}

impl Primitive {
    /// Owning structure used for model-to-world placement.
    #[must_use]
    pub const fn owner(self) -> StructureHandle {
        match self {
            Self::Ellipsoid { owner, .. }
            | Self::Planar {
                value: crate::PlanarRegion { owner, .. },
                ..
            } => owner,
            Self::Carbohydrate(value) => value.owner,
            Self::Particle(value) => value.owner,
        }
    }

    /// Whether the primitive is active.
    #[must_use]
    pub const fn visible(self) -> bool {
        match self {
            Self::Ellipsoid { visible, .. } | Self::Planar { visible, .. } => visible,
            Self::Carbohydrate(value) => value.visible,
            Self::Particle(value) => value.visible,
        }
    }

    /// Conservative model-space bounds.
    #[must_use]
    pub fn bounds(self) -> Aabb {
        match self {
            Self::Ellipsoid { value, .. } => value.bounds(),
            Self::Carbohydrate(value) => value.bounds(),
            Self::Planar { value, .. } => Aabb::from_points(value.corners()),
            Self::Particle(value) => value.bounds(),
        }
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}

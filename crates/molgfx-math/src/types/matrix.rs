//! Project-owned quaternions and matrices lowered to glam operations.

use std::ops::Mul;

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

use super::{Vec3, Vec4};

/// A SIMD-aligned quaternion stored as `(x, y, z, w)`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[serde(from = "[f32; 4]", into = "[f32; 4]")]
pub struct Quat {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
    /// Scalar component.
    pub w: f32,
}

impl Quat {
    /// Identity rotation.
    pub const IDENTITY: Self = Self::from_xyzw(0.0, 0.0, 0.0, 1.0);
    /// Builds a quaternion.
    #[must_use]
    #[inline]
    pub const fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    /// Builds from an array.
    #[must_use]
    #[inline]
    pub const fn from_array(value: [f32; 4]) -> Self {
        Self::from_xyzw(value[0], value[1], value[2], value[3])
    }
    /// Returns components.
    #[must_use]
    #[inline]
    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Rotation around X.
    #[must_use]
    #[inline]
    pub fn from_rotation_x(angle: f32) -> Self {
        glam::Quat::from_rotation_x(angle).into()
    }
    /// Rotation around Y.
    #[must_use]
    #[inline]
    pub fn from_rotation_y(angle: f32) -> Self {
        glam::Quat::from_rotation_y(angle).into()
    }
    /// Rotation around Z.
    #[must_use]
    #[inline]
    pub fn from_rotation_z(angle: f32) -> Self {
        glam::Quat::from_rotation_z(angle).into()
    }
    /// Rotation around a normalized axis.
    #[must_use]
    #[inline]
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        glam::Quat::from_axis_angle(axis.into(), angle).into()
    }
    /// Shortest rotation between directions.
    #[must_use]
    #[inline]
    pub fn from_rotation_arc(from: Vec3, to: Vec3) -> Self {
        glam::Quat::from_rotation_arc(from.into(), to.into()).into()
    }
    /// Builds from a rotation matrix.
    #[must_use]
    #[inline]
    pub fn from_mat3(matrix: &Mat3) -> Self {
        glam::Quat::from_mat3(&(*matrix).into()).into()
    }
    /// Spherical interpolation.
    #[must_use]
    #[inline]
    pub fn slerp(self, rhs: Self, amount: f32) -> Self {
        glam::Quat::from(self).slerp(rhs.into(), amount).into()
    }
    /// Returns whether every component is finite.
    #[must_use]
    #[inline]
    pub fn is_finite(self) -> bool {
        glam::Quat::from(self).is_finite()
    }
    /// Squared quaternion length.
    #[must_use]
    #[inline]
    pub fn length_squared(self) -> f32 {
        glam::Quat::from(self).length_squared()
    }
    /// Returns a normalized quaternion.
    #[must_use]
    #[inline]
    pub fn normalize(self) -> Self {
        glam::Quat::from(self).normalize().into()
    }
}

impl Default for Quat {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl From<[f32; 4]> for Quat {
    #[inline]
    fn from(value: [f32; 4]) -> Self {
        Self::from_array(value)
    }
}
impl From<Quat> for [f32; 4] {
    #[inline]
    fn from(value: Quat) -> Self {
        value.to_array()
    }
}
impl From<glam::Quat> for Quat {
    #[inline]
    fn from(value: glam::Quat) -> Self {
        value.to_array().into()
    }
}
impl From<Quat> for glam::Quat {
    #[inline]
    fn from(value: Quat) -> Self {
        Self::from_array(value.to_array())
    }
}
impl Mul for Quat {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (glam::Quat::from(self) * glam::Quat::from(rhs)).into()
    }
}
impl Mul<Vec3> for Quat {
    type Output = Vec3;
    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        (glam::Quat::from(self) * glam::Vec3::from(rhs)).into()
    }
}

/// A column-major 3×3 matrix.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Mat3 {
    /// First column.
    pub x_axis: Vec3,
    /// Second column.
    pub y_axis: Vec3,
    /// Third column.
    pub z_axis: Vec3,
}

impl Mat3 {
    /// Identity matrix.
    pub const IDENTITY: Self = Self::from_cols(Vec3::X, Vec3::Y, Vec3::Z);
    /// Builds from columns.
    #[must_use]
    #[inline]
    pub const fn from_cols(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
        }
    }
    /// Extracts the linear part of a 4×4 matrix.
    #[must_use]
    #[inline]
    pub fn from_mat4(value: Mat4) -> Self {
        glam::Mat3::from_mat4(value.into()).into()
    }
    /// Builds from a quaternion.
    #[must_use]
    #[inline]
    pub fn from_quat(value: Quat) -> Self {
        glam::Mat3::from_quat(value.into()).into()
    }
    /// Matrix inverse.
    #[must_use]
    #[inline]
    pub fn inverse(self) -> Self {
        glam::Mat3::from(self).inverse().into()
    }
    /// Matrix transpose.
    #[must_use]
    #[inline]
    pub fn transpose(self) -> Self {
        glam::Mat3::from(self).transpose().into()
    }
    /// Determinant.
    #[must_use]
    #[inline]
    pub fn determinant(self) -> f32 {
        glam::Mat3::from(self).determinant()
    }
    /// Returns whether all entries are finite.
    #[must_use]
    #[inline]
    pub fn is_finite(self) -> bool {
        glam::Mat3::from(self).is_finite()
    }
}

impl From<glam::Mat3> for Mat3 {
    #[inline]
    fn from(value: glam::Mat3) -> Self {
        Self::from_cols(
            value.x_axis.into(),
            value.y_axis.into(),
            value.z_axis.into(),
        )
    }
}
impl From<Mat3> for glam::Mat3 {
    #[inline]
    fn from(value: Mat3) -> Self {
        Self::from_cols(
            value.x_axis.into(),
            value.y_axis.into(),
            value.z_axis.into(),
        )
    }
}
impl Mul for Mat3 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (glam::Mat3::from(self) * glam::Mat3::from(rhs)).into()
    }
}
impl Mul<Vec3> for Mat3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        (glam::Mat3::from(self) * glam::Vec3::from(rhs)).into()
    }
}

/// A SIMD-aligned column-major 4×4 matrix.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
pub struct Mat4 {
    /// First column.
    pub x_axis: Vec4,
    /// Second column.
    pub y_axis: Vec4,
    /// Third column.
    pub z_axis: Vec4,
    /// Fourth column.
    pub w_axis: Vec4,
}

impl Mat4 {
    /// Identity matrix.
    pub const IDENTITY: Self = Self::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    /// Zero matrix.
    pub const ZERO: Self = Self::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO);
    /// Builds from columns.
    #[must_use]
    #[inline]
    pub const fn from_cols(x_axis: Vec4, y_axis: Vec4, z_axis: Vec4, w_axis: Vec4) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
            w_axis,
        }
    }
    /// Builds from column-major elements.
    #[must_use]
    #[inline]
    pub fn from_cols_array(value: &[f32; 16]) -> Self {
        glam::Mat4::from_cols_array(value).into()
    }
    /// Returns column-major elements.
    #[must_use]
    #[inline]
    pub fn to_cols_array(self) -> [f32; 16] {
        glam::Mat4::from(self).to_cols_array()
    }
    /// Returns a column-major nested array.
    #[must_use]
    #[inline]
    pub fn to_cols_array_2d(self) -> [[f32; 4]; 4] {
        glam::Mat4::from(self).to_cols_array_2d()
    }
    /// Translation matrix.
    #[must_use]
    #[inline]
    pub fn from_translation(value: Vec3) -> Self {
        glam::Mat4::from_translation(value.into()).into()
    }
    /// Rotation around Y.
    #[must_use]
    #[inline]
    pub fn from_rotation_y(angle: f32) -> Self {
        glam::Mat4::from_rotation_y(angle).into()
    }
    /// Scale, rotation and translation matrix.
    #[must_use]
    #[inline]
    pub fn from_scale_rotation_translation(scale: Vec3, rotation: Quat, translation: Vec3) -> Self {
        glam::Mat4::from_scale_rotation_translation(
            scale.into(),
            rotation.into(),
            translation.into(),
        )
        .into()
    }
    /// Transforms a point.
    #[must_use]
    #[inline]
    pub fn transform_point3(self, point: Vec3) -> Vec3 {
        glam::Mat4::from(self).transform_point3(point.into()).into()
    }
    /// Transforms a direction.
    #[must_use]
    #[inline]
    pub fn transform_vector3(self, vector: Vec3) -> Vec3 {
        glam::Mat4::from(self)
            .transform_vector3(vector.into())
            .into()
    }
    /// Matrix inverse.
    #[must_use]
    #[inline]
    pub fn inverse(self) -> Self {
        glam::Mat4::from(self).inverse().into()
    }
    /// Determinant.
    #[must_use]
    #[inline]
    pub fn determinant(self) -> f32 {
        glam::Mat4::from(self).determinant()
    }
    /// Returns whether all entries are finite.
    #[must_use]
    #[inline]
    pub fn is_finite(self) -> bool {
        glam::Mat4::from(self).is_finite()
    }
    /// Returns a row.
    #[must_use]
    #[inline]
    pub fn row(self, index: usize) -> Vec4 {
        glam::Mat4::from(self).row(index).into()
    }
}

impl Default for Mat4 {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl From<glam::Mat4> for Mat4 {
    #[inline]
    fn from(value: glam::Mat4) -> Self {
        Self::from_cols(
            value.x_axis.into(),
            value.y_axis.into(),
            value.z_axis.into(),
            value.w_axis.into(),
        )
    }
}
impl From<Mat4> for glam::Mat4 {
    #[inline]
    fn from(value: Mat4) -> Self {
        Self::from_cols(
            value.x_axis.into(),
            value.y_axis.into(),
            value.z_axis.into(),
            value.w_axis.into(),
        )
    }
}
impl Mul for Mat4 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (glam::Mat4::from(self) * glam::Mat4::from(rhs)).into()
    }
}
impl Mul<Vec4> for Mat4 {
    type Output = Vec4;
    #[inline]
    fn mul(self, rhs: Vec4) -> Vec4 {
        (glam::Mat4::from(self) * glam::Vec4::from(rhs)).into()
    }
}

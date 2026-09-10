//! Project-owned vectors lowered to glam operations.

use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

macro_rules! component_ops {
    ($name:ident { $($field:ident),+ }) => {
        impl Add for $name {
            type Output = Self;
            #[inline] fn add(self, rhs: Self) -> Self { Self { $($field: self.$field + rhs.$field),+ } }
        }
        impl Sub for $name {
            type Output = Self;
            #[inline] fn sub(self, rhs: Self) -> Self { Self { $($field: self.$field - rhs.$field),+ } }
        }
        impl Mul for $name {
            type Output = Self;
            #[inline] fn mul(self, rhs: Self) -> Self { Self { $($field: self.$field * rhs.$field),+ } }
        }
        impl Div for $name {
            type Output = Self;
            #[inline] fn div(self, rhs: Self) -> Self { Self { $($field: self.$field / rhs.$field),+ } }
        }
        impl Mul<f32> for $name {
            type Output = Self;
            #[inline] fn mul(self, rhs: f32) -> Self { Self { $($field: self.$field * rhs),+ } }
        }
        impl Div<f32> for $name {
            type Output = Self;
            #[inline] fn div(self, rhs: f32) -> Self { Self { $($field: self.$field / rhs),+ } }
        }
        impl Neg for $name {
            type Output = Self;
            #[inline] fn neg(self) -> Self { Self { $($field: -self.$field),+ } }
        }
        impl AddAssign for $name { #[inline] fn add_assign(&mut self, rhs: Self) { *self = *self + rhs; } }
        impl SubAssign for $name { #[inline] fn sub_assign(&mut self, rhs: Self) { *self = *self - rhs; } }
        impl MulAssign<f32> for $name { #[inline] fn mul_assign(&mut self, rhs: f32) { *self = *self * rhs; } }
        impl DivAssign<f32> for $name { #[inline] fn div_assign(&mut self, rhs: f32) { *self = *self / rhs; } }
    };
}

/// A two-component single-precision vector.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[serde(from = "[f32; 2]", into = "[f32; 2]")]
pub struct Vec2 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
}

impl Vec2 {
    /// Zero vector.
    pub const ZERO: Self = Self::splat(0.0);
    /// Builds a vector.
    #[must_use]
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    /// Builds a vector with equal components.
    #[must_use]
    #[inline]
    pub const fn splat(value: f32) -> Self {
        Self::new(value, value)
    }
    /// Returns components.
    #[must_use]
    #[inline]
    pub const fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}

impl From<[f32; 2]> for Vec2 {
    #[inline]
    fn from(value: [f32; 2]) -> Self {
        Self::new(value[0], value[1])
    }
}
impl From<Vec2> for [f32; 2] {
    #[inline]
    fn from(value: Vec2) -> Self {
        value.to_array()
    }
}
impl From<glam::Vec2> for Vec2 {
    #[inline]
    fn from(value: glam::Vec2) -> Self {
        value.to_array().into()
    }
}
impl From<Vec2> for glam::Vec2 {
    #[inline]
    fn from(value: Vec2) -> Self {
        Self::from_array(value.to_array())
    }
}
component_ops!(Vec2 { x, y });

/// A three-component single-precision vector.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[serde(from = "[f32; 3]", into = "[f32; 3]")]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

impl Vec3 {
    /// Zero vector.
    pub const ZERO: Self = Self::splat(0.0);
    /// One on every axis.
    pub const ONE: Self = Self::splat(1.0);
    /// Positive X axis.
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    /// Positive Y axis.
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    /// Positive Z axis.
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);
    /// NaN on every axis.
    pub const NAN: Self = Self::splat(f32::NAN);
    /// Builds a vector.
    #[must_use]
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    /// Builds a vector with equal components.
    #[must_use]
    #[inline]
    pub const fn splat(value: f32) -> Self {
        Self::new(value, value, value)
    }
    /// Builds from an array.
    #[must_use]
    #[inline]
    pub const fn from_array(value: [f32; 3]) -> Self {
        Self::new(value[0], value[1], value[2])
    }
    /// Builds from the first three slice entries.
    #[must_use]
    #[inline]
    pub fn from_slice(value: &[f32]) -> Self {
        Self::from(glam::Vec3::from_slice(value))
    }
    /// Returns components.
    #[must_use]
    #[inline]
    pub const fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
    /// Returns whether every component is finite.
    #[must_use]
    #[inline]
    pub fn is_finite(self) -> bool {
        glam::Vec3::from(self).is_finite()
    }
    /// Returns whether the length is approximately one.
    #[must_use]
    #[inline]
    pub fn is_normalized(self) -> bool {
        glam::Vec3::from(self).is_normalized()
    }
    /// Dot product.
    #[must_use]
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        glam::Vec3::from(self).dot(rhs.into())
    }
    /// Cross product.
    #[must_use]
    #[inline]
    pub fn cross(self, rhs: Self) -> Self {
        glam::Vec3::from(self).cross(rhs.into()).into()
    }
    /// Squared length.
    #[must_use]
    #[inline]
    pub fn length_squared(self) -> f32 {
        glam::Vec3::from(self).length_squared()
    }
    /// Length.
    #[must_use]
    #[inline]
    pub fn length(self) -> f32 {
        glam::Vec3::from(self).length()
    }
    /// Squared point distance.
    #[must_use]
    #[inline]
    pub fn distance_squared(self, rhs: Self) -> f32 {
        glam::Vec3::from(self).distance_squared(rhs.into())
    }
    /// Point distance.
    #[must_use]
    #[inline]
    pub fn distance(self, rhs: Self) -> f32 {
        glam::Vec3::from(self).distance(rhs.into())
    }
    /// Normalized vector.
    #[must_use]
    #[inline]
    pub fn normalize(self) -> Self {
        glam::Vec3::from(self).normalize().into()
    }
    /// Normalized vector, or zero when degenerate.
    #[must_use]
    #[inline]
    pub fn normalize_or_zero(self) -> Self {
        glam::Vec3::from(self).normalize_or_zero().into()
    }
    /// Attempts normalization.
    #[must_use]
    #[inline]
    pub fn try_normalize(self) -> Option<Self> {
        glam::Vec3::from(self).try_normalize().map(Into::into)
    }
    /// Componentwise absolute value.
    #[must_use]
    #[inline]
    pub fn abs(self) -> Self {
        glam::Vec3::from(self).abs().into()
    }
    /// Componentwise square root.
    #[must_use]
    #[inline]
    pub fn sqrt(self) -> Self {
        glam::Vec3::from(self).sqrt().into()
    }
    /// Componentwise reciprocal.
    #[must_use]
    #[inline]
    pub fn recip(self) -> Self {
        glam::Vec3::from(self).recip().into()
    }
    /// Componentwise minimum.
    #[must_use]
    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        glam::Vec3::from(self).min(rhs.into()).into()
    }
    /// Componentwise maximum.
    #[must_use]
    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        glam::Vec3::from(self).max(rhs.into()).into()
    }
    /// Componentwise clamp.
    #[must_use]
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        glam::Vec3::from(self).clamp(min.into(), max.into()).into()
    }
    /// Smallest component.
    #[must_use]
    #[inline]
    pub fn min_element(self) -> f32 {
        glam::Vec3::from(self).min_element()
    }
    /// Largest component.
    #[must_use]
    #[inline]
    pub fn max_element(self) -> f32 {
        glam::Vec3::from(self).max_element()
    }
    /// Linear interpolation.
    #[must_use]
    #[inline]
    pub fn lerp(self, rhs: Self, amount: f32) -> Self {
        glam::Vec3::from(self).lerp(rhs.into(), amount).into()
    }
    /// Rejects this vector from a normalized direction.
    #[must_use]
    #[inline]
    pub fn reject_from_normalized(self, normal: Self) -> Self {
        glam::Vec3::from(self)
            .reject_from_normalized(normal.into())
            .into()
    }
    /// Returns a deterministic orthonormal vector.
    #[must_use]
    #[inline]
    pub fn any_orthonormal_vector(self) -> Self {
        glam::Vec3::from(self).any_orthonormal_vector().into()
    }
    /// Appends a W component.
    #[must_use]
    #[inline]
    pub const fn extend(self, w: f32) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, w)
    }
}

impl From<[f32; 3]> for Vec3 {
    #[inline]
    fn from(value: [f32; 3]) -> Self {
        Self::from_array(value)
    }
}
impl From<Vec3> for [f32; 3] {
    #[inline]
    fn from(value: Vec3) -> Self {
        value.to_array()
    }
}
impl From<glam::Vec3> for Vec3 {
    #[inline]
    fn from(value: glam::Vec3) -> Self {
        value.to_array().into()
    }
}
impl From<Vec3> for glam::Vec3 {
    #[inline]
    fn from(value: Vec3) -> Self {
        Self::from_array(value.to_array())
    }
}
impl From<&[f32; 3]> for Vec3 {
    #[inline]
    fn from(value: &[f32; 3]) -> Self {
        Self::from_array(*value)
    }
}
component_ops!(Vec3 { x, y, z });
impl Mul<Vec3> for f32 {
    type Output = Vec3;
    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        rhs * self
    }
}
impl Sum for Vec3 {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

/// A four-component SIMD-aligned single-precision vector.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[serde(from = "[f32; 4]", into = "[f32; 4]")]
pub struct Vec4 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
    /// W component.
    pub w: f32,
}

impl Vec4 {
    /// Zero vector.
    pub const ZERO: Self = Self::splat(0.0);
    /// Builds a vector.
    #[must_use]
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    /// Builds a vector with equal components.
    #[must_use]
    #[inline]
    pub const fn splat(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
    /// Returns components.
    #[must_use]
    #[inline]
    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    /// Returns XYZ.
    #[must_use]
    #[inline]
    pub const fn truncate(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
    /// Vector length.
    #[must_use]
    #[inline]
    pub fn length(self) -> f32 {
        glam::Vec4::from(self).length()
    }
    /// Returns whether every component is finite.
    #[must_use]
    #[inline]
    pub fn is_finite(self) -> bool {
        glam::Vec4::from(self).is_finite()
    }
}

impl From<[f32; 4]> for Vec4 {
    #[inline]
    fn from(value: [f32; 4]) -> Self {
        Self::new(value[0], value[1], value[2], value[3])
    }
}
impl From<Vec4> for [f32; 4] {
    #[inline]
    fn from(value: Vec4) -> Self {
        value.to_array()
    }
}
impl From<glam::Vec4> for Vec4 {
    #[inline]
    fn from(value: glam::Vec4) -> Self {
        value.to_array().into()
    }
}
impl From<Vec4> for glam::Vec4 {
    #[inline]
    fn from(value: Vec4) -> Self {
        Self::from_array(value.to_array())
    }
}
component_ops!(Vec4 { x, y, z, w });

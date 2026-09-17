//! Constructors and queries for representation colour schemes.

use super::ColorScheme;
use crate::AtomPropertyHandle;
use molgfx_math::Rgba8;

impl ColorScheme {
    /// Builds a sequential property colour scheme and its matching legend.
    #[must_use]
    pub fn property(property: AtomPropertyHandle, value: &crate::AtomProperty) -> Self {
        Self::ByProperty {
            property,
            ramp: crate::ScalarRamp::sequential(value.display_domain()),
            missing: Rgba8::opaque(112, 112, 112),
        }
    }

    /// Referenced property column, when this is a continuous encoding.
    #[must_use]
    pub const fn property_handle(self) -> Option<AtomPropertyHandle> {
        match self {
            Self::ByProperty { property, .. } => Some(property),
            _ => None,
        }
    }
}

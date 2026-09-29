//! Constructors and queries for representation colour schemes.

use super::ColorScheme;
use crate::AtomPropertyHandle;
use molgfx_math::Rgba8;

impl ColorScheme {
    /// Colours a categorical column through `palette`.
    #[must_use]
    pub const fn category(property: AtomPropertyHandle, palette: crate::CategoryPalette) -> Self {
        Self::ByCategory { property, palette }
    }

    /// Builds a sequential property colour scheme and its matching legend.
    #[must_use]
    pub fn property(property: AtomPropertyHandle, value: &crate::AtomProperty) -> Self {
        Self::ByProperty {
            property,
            ramp: crate::ScalarRamp::sequential(value.display_domain()),
            missing: Rgba8::opaque(112, 112, 112),
        }
    }

    /// Referenced property column, when the colour is driven by one.
    #[must_use]
    pub const fn property_handle(self) -> Option<AtomPropertyHandle> {
        match self {
            Self::ByProperty { property, .. } | Self::ByCategory { property, .. } => Some(property),
            _ => None,
        }
    }
}

/// Every property column that colours one representation.
///
/// Planning which columns to upload, resolving their arena offsets and
/// detecting which column edits invalidate baked geometry all need the same
/// list, so it is derived here once.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ColorColumns {
    /// The base scheme's column: a ramp or a category.
    pub base: Option<AtomPropertyHandle>,
    /// The overlay's class column.
    pub overlay_classes: Option<AtomPropertyHandle>,
    /// The column each overlay scheme reads, class one first.
    pub overlay_schemes: [Option<AtomPropertyHandle>; crate::MAX_COLOR_OVERLAY_CLASSES],
    /// The appearance mapping's column.
    pub appearance: Option<AtomPropertyHandle>,
}

impl ColorColumns {
    /// Every distinct handle, in a stable order.
    pub fn handles(&self) -> impl Iterator<Item = AtomPropertyHandle> + '_ {
        [self.base, self.overlay_classes, self.appearance]
            .into_iter()
            .chain(self.overlay_schemes)
            .flatten()
    }
}

impl super::Representation {
    /// The property columns this representation's colour reads.
    #[must_use]
    pub fn color_columns(&self) -> ColorColumns {
        let mut columns = ColorColumns {
            base: self.color.property_handle(),
            appearance: self.appearance.map(|appearance| appearance.property),
            ..ColorColumns::default()
        };
        if let Some(overlay) = self.color_overlay {
            columns.overlay_classes = Some(overlay.classes());
            for (slot, scheme) in columns.overlay_schemes.iter_mut().zip(overlay.schemes()) {
                *slot = scheme.property_handle();
            }
        }
        columns
    }
}

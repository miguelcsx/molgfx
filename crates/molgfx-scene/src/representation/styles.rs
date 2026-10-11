//! Style and kind selectors for the representation forms.

/// Cartoon geometry recipe.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartoonStyle {
    /// Smooth secondary-structure ribbon.
    Ribbon,
    /// Discrete helix and strand solids.
    Rocket,
    /// Nucleic-acid backbone and base-aware ribbon.
    NucleicAcid,
    /// Glycosidic tree ribbon.
    Glycan,
}

/// Molecular-surface definition.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// Van der Waals boundary.
    VanDerWaals,
    /// Solvent-accessible boundary.
    SolventAccessible,
    /// Solvent-excluded boundary.
    SolventExcluded,
    /// Gaussian density boundary.
    Gaussian,
}

/// Molecular-surface presentation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceStyle {
    /// Filled boundary.
    Solid,
    /// Contour lines.
    Contour,
    /// Dot lattice.
    Dots,
    /// Filled boundary with contours.
    FilledContour,
    /// Wire lattice.
    Mesh,
    /// Smoothed soft-minimum union of the contributing atoms, the blob
    /// surface both reference engines show for a rounded molecular envelope.
    SoftUnion,
}

impl SurfaceKind {
    pub(crate) fn native(self) -> molgfx_core::SurfaceKind {
        match self {
            SurfaceKind::SolventExcluded => molgfx_core::SurfaceKind::SolventExcluded,
            SurfaceKind::VanDerWaals => molgfx_core::SurfaceKind::VanDerWaals,
            SurfaceKind::SolventAccessible => molgfx_core::SurfaceKind::SolventAccessible,
            SurfaceKind::Gaussian => molgfx_core::SurfaceKind::Gaussian,
        }
    }
}

impl SurfaceStyle {
    pub(crate) fn native(self) -> molgfx_core::SurfaceStyle {
        match self {
            SurfaceStyle::Solid => molgfx_core::SurfaceStyle::Solid,
            SurfaceStyle::Contour => molgfx_core::SurfaceStyle::Contour,
            SurfaceStyle::Dots => molgfx_core::SurfaceStyle::Dots,
            SurfaceStyle::FilledContour => molgfx_core::SurfaceStyle::FilledContour,
            SurfaceStyle::Mesh => molgfx_core::SurfaceStyle::Mesh,
            SurfaceStyle::SoftUnion => molgfx_core::SurfaceStyle::SoftUnion,
        }
    }
}

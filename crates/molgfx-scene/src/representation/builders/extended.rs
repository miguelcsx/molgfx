//! Molecular forms with independent geometry controls.

use super::{
    CartoonProfile, ColorSpec, Common, GapStyle, RepresentationFormSpec, RepresentationSpec,
    SceneItem, Selection, private,
};

/// Peptide-oriented secondary-structure ribbons following the curved backbone.
#[must_use]
pub fn new_cartoon(target: impl Into<Selection>) -> super::Cartoon {
    super::Cartoon::new(target)
        .width(1.2)
        .style(super::CartoonStyle::Ribbon)
}

representation! {
    /// Bond capsules without spheres, including at isolated atoms.
    Bonds => Bonds {
        /// Sets capsule radius in ångström.
        bond_radius => bond_radius: f32 = 0.25,
    }
}

representation! {
    /// Pucker-coloured molecular ring bipyramids.
    PaperChain => PaperChain {
        /// Sets total bipyramid height in ångström.
        height => height: f32 = 1.0,
        /// Sets the largest ring, between three and twelve atoms.
        max_ring_size => max_ring_size: u8 = 6,
    }
}

/// Bond capsules without atom junction spheres.
#[must_use]
pub fn bonds(target: impl Into<Selection>) -> Bonds {
    Bonds::new(target)
}

/// Pucker-coloured molecular ring bipyramids.
#[must_use]
pub fn paper_chain(target: impl Into<Selection>) -> PaperChain {
    PaperChain::new(target)
}

representation! {
    /// Constant-width peptide-oriented ribbon, independent of secondary structure.
    NewRibbons => NewRibbons {
        /// Sets full width in ångström.
        width => width: f32 = 1.2,
        /// Sets width divided by thickness.
        aspect_ratio => aspect_ratio: f32 = 5.0,
        /// Sets the extruded cross-section.
        profile => profile: CartoonProfile = CartoonProfile::Elliptical,
        /// Sets presentation of real polymer gaps.
        gaps => gaps: GapStyle = GapStyle::Hidden,
    }
}

/// Constant-width peptide-oriented polymer ribbon.
#[must_use]
pub fn new_ribbons(target: impl Into<Selection>) -> NewRibbons {
    NewRibbons::new(target)
}

representation! {
    /// Triplets of compatible neighbors around selected central atoms.
    Polyhedra => Polyhedra {
        /// Sets the central-atom-to-neighbor cutoff in ångström.
        cutoff => cutoff: f32 = 3.0,
    }
}

/// Neighbor triangles around every selected central atom.
#[must_use]
pub fn polyhedra(target: impl Into<Selection>) -> Polyhedra {
    Polyhedra::new(target)
}

representation! {
    /// Distance-only links; these never replace the molecular bond topology.
    DynamicBonds => DynamicBonds {
        /// Sets the instantaneous pairwise cutoff in ångström.
        cutoff => cutoff: f32 = 1.6,
        /// Sets the capsule radius in ångström.
        bond_radius => bond_radius: f32 = 0.25,
    }
}

/// Distance-only links evaluated on the current interpolated coordinates.
#[must_use]
pub fn dynamic_bonds(target: impl Into<Selection>) -> DynamicBonds {
    DynamicBonds::new(target)
}

representation! {
    /// Solvent-accessible sphere samples with overlap rejected on the GPU.
    Solvent => Solvent {
        /// Sets solvent probe radius in ångström.
        probe_radius => probe_radius: f32 = 1.4,
        /// Sets sample diameter in physical pixels.
        size => size: f32 = 1.0,
        /// Sets samples per probe-inflated sphere, between one and 4096.
        samples => samples: u16 = 64,
    }
}

/// Probe-inflated atomic samples on the accessible solvent boundary.
#[must_use]
pub fn solvent(target: impl Into<Selection>) -> Solvent {
    Solvent::new(target)
}

representation! {
    /// Two independently splined border tubes joined by a thin sheet.
    Ribbons => Ribbons {
        /// Sets the full guide width in ångström.
        width => width: f32 = 1.2,
        /// Sets the border tube radius; zero draws only the sheet.
        edge_radius => edge_radius: f32 = 0.15,
        /// Sets presentation of real polymer gaps.
        gaps => gaps: GapStyle = GapStyle::Hidden,
    }
}

/// Flat polymer sheet bounded by independently splined circular tubes.
#[must_use]
pub fn ribbons(target: impl Into<Selection>) -> Ribbons {
    Ribbons::new(target)
}

representation! {
    /// Iso-density boundary of summed atom-centred Gaussians.
    QuickSurf => QuickSurf {
        /// Scales van der Waals atomic radii.
        radius => radius: f32 = 1.0,
        /// Scales atomic radii into Gaussian standard deviations.
        sigma => sigma: f32 = 1.0,
        /// Sets positive density threshold.
        isolevel => isolevel: f32 = 0.5,
        /// Requests field spacing in ångström, subject to device and memory limits.
        grid_spacing => grid_spacing: f32 = 0.5,
        /// Limits support in standard deviations, controlling truncation error.
        cutoff => cutoff: f32 = 4.0,
    }
}
/// Smooth atom-centred Gaussian density boundary.
#[must_use]
pub fn quick_surf(target: impl Into<Selection>) -> QuickSurf {
    QuickSurf::new(target)
}

representation! {
    /// Analytical provider triangles with original molecular atom identities.
    MolecularSurface => MolecularSurface {
        /// Declares the bulk geometry source supplied through a runtime binding.
        source => source: crate::DataSource = crate::DataSource::new(""),
        /// Chooses the actual scientific provider.
        method => method: super::super::MolecularSurfaceMethod = super::super::MolecularSurfaceMethod::Surf,
        /// Sets the scientific controls used by the provider.
        controls => controls: super::super::MolecularSurfaceControls = super::super::MolecularSurfaceControls::default(),
        /// Displays the provider's actual triangle edges in physical pixels.
        wire => wire: bool = false,
        /// Sets triangle-edge width in physical pixels.
        width => width: f32 = 1.5,
    }
}

/// Varshney rolling-probe triangles; requires a matching runtime surface binding.
#[must_use]
pub fn surf(target: impl Into<Selection>, source: crate::DataSource) -> MolecularSurface {
    MolecularSurface::new(target).source(source)
}

/// Sanner rolling-probe triangulation; requires a matching runtime surface binding.
#[must_use]
pub fn msms(target: impl Into<Selection>, source: crate::DataSource) -> MolecularSurface {
    MolecularSurface::new(target)
        .source(source)
        .method(super::super::MolecularSurfaceMethod::Msms)
}

impl MolecularSurface {
    /// Sets rolling solvent-probe radius in ångström.
    #[must_use]
    pub fn probe_radius(mut self, radius: f32) -> Self {
        self.controls.probe_radius = radius;
        self
    }
    /// Sets MSMS vertices per square ångström.
    #[must_use]
    pub fn density(mut self, density: f32) -> Self {
        self.controls.density = density;
        self
    }
    /// Selects whether MSMS includes disconnected components and cavities.
    #[must_use]
    pub fn all_components(mut self, include: bool) -> Self {
        self.controls.all_components = include;
        self
    }
}

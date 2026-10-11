//! Molecular forms with independent geometry controls.

use super::{
    ColorSpec, Common, RepresentationFormSpec, RepresentationSpec, SceneItem, Selection, private,
};

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
        /// Sets ring pyramid thickness in ångström.
        width => width: f32 = 1.0,
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

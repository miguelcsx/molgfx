//! Mutable semantic scene and atomic transaction boundary.

pub use assembly::{AssemblyCopy, chain_selection};
pub use inspect::{
    ResidueMetadata, ResolvedAtomPick, ResolvedBondPick, ResolvedLabelPick,
    ResolvedMeasurementPick, ResolvedPick, ResolvedVolumeSegmentPick,
};

pub(crate) mod appearance;
pub(crate) mod apply;
mod assembly;
mod atom_pick;
pub(crate) mod domains;
#[cfg(test)]
mod domains_tests;
mod framing;
pub(crate) mod hashing;
mod insertion;
mod inspect;
pub(crate) mod interaction;
#[cfg(test)]
mod interaction_tests;
mod operations;
mod overlay;
mod overlay_pick;
mod presets;
pub(crate) mod properties;
pub(crate) mod runtime;
#[cfg(test)]
mod runtime_tests;
pub(crate) mod selection_rows;
mod structures;
pub(crate) mod transaction;

#[cfg(test)]
mod inspect_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod tests;

mod accessors;
mod construction;
mod derived_bindings;
mod editing;
mod items;
mod state;

use derived_bindings::with_derived_bindings;
pub(crate) use state::Resolution;
pub use state::Scene;

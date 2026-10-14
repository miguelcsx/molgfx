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
mod framing;
pub(crate) mod hashing;
mod hydrogen_bonds;
mod insertion;
mod inspect;
pub(crate) mod interaction;
#[cfg(test)]
mod interaction_tests;
mod molecular_surface;
mod operations;
mod overlay;
mod overlay_pick;
mod pointer_hover;
mod presets;
pub(crate) mod properties;
pub(crate) mod runtime;
#[cfg(test)]
mod runtime_tests;
pub(crate) mod selection_rows;
pub(crate) mod snfg_symbols;
mod structures;
mod timeline;
#[cfg(test)]
mod track_input_tests;
pub(crate) mod transaction;
#[cfg(test)]
mod volume_binding_tests;

#[cfg(test)]
mod inspect_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod tests;

mod accessors;
mod binding_retention;
mod bulk_bindings;
mod playback_binding;
mod playback_resolution;
pub use playback_binding::PlaybackBinding;
mod runtime_bindings;
#[cfg(test)]
mod runtime_bindings_tests;
mod runtime_capture;
mod selection_scope;
pub use runtime_bindings::RuntimeBindings;
mod construction;
mod derived_bindings;
mod editing;
mod ensemble;
#[cfg(test)]
mod ensemble_tests;
mod items;
#[cfg(test)]
mod molecular_surface_tests;
#[cfg(test)]
mod representation_edit_tests;
mod restyle;
#[cfg(test)]
mod restyle_tests;
mod snapshot;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod snfg_tests;
mod state;

use derived_bindings::with_derived_bindings;
pub(crate) use state::Resolution;
pub use state::Scene;
mod segmentation;
#[cfg(test)]
mod segmentation_tests;

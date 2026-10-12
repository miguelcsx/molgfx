//! Polymer cartoon and nucleotide geometry.

mod base_pairs;
mod cross_section;
pub use base_pairs::paired_base_selection;
mod directions;
mod draw_limits;
pub(crate) mod ends;
mod error;
mod gaps;
mod glycan;
pub(crate) mod nucleic;
mod orientation;
pub(crate) mod paper_chain;
pub(crate) mod profiles;
pub(crate) mod ribbon;
mod rocket;
pub(crate) mod secondary_motion;
mod selection_span;
mod stroke;
mod sweep;
pub(crate) mod traces;
pub(crate) mod twist;

pub use error::CartoonError;
pub use glycan::extract_glycosidic_traces;
pub use nucleic::{append_base_polygons, append_base_slabs};
pub use paper_chain::append_paper_chain;
pub use profiles::variable_tube_radius;
pub use ribbon::{RibbonMesh, RibbonParams, RibbonVertex, SplineProfile, has_selected_sugar_faces};
pub use secondary_motion::{SecondaryMotion, solve_offsets};
pub use traces::{
    CARTOON_GAP_CUTOFF, PolymerTraces, TraceRange, extract_contextual_polymer_traces,
    extract_phosphate_traces, extract_polymer_traces, extract_protein_traces,
};

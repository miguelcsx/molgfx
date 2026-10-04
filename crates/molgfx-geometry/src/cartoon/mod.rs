//! Polymer cartoon and nucleotide geometry.

mod cross_section;
mod directions;
mod draw_limits;
pub(crate) mod ends;
mod error;
mod glycan;
pub(crate) mod nucleic;
pub(crate) mod paper_chain;
pub(crate) mod profiles;
pub(crate) mod ribbon;
pub(crate) mod secondary_motion;
mod sweep;
pub(crate) mod traces;
pub(crate) mod twist;

pub use error::CartoonError;
pub use glycan::extract_glycosidic_traces;
pub use nucleic::{append_base_polygons, append_base_slabs};
pub use paper_chain::append_paper_chain;
pub use profiles::variable_tube_radius;
pub use ribbon::{RibbonMesh, RibbonParams, RibbonVertex, SplineProfile};
pub use secondary_motion::{SecondaryMotion, solve_offsets};
pub use traces::{CARTOON_GAP_CUTOFF, PolymerTraces, TraceRange, extract_polymer_traces};

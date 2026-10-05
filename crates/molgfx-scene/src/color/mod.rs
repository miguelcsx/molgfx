//! Immutable color specifications.

mod derived;
pub mod ramps;
mod spec;
#[cfg(test)]
mod tests;

pub use derived::{AtomCategory, AtomMetric, DerivedColumn};
pub use spec::ColorSpec;

pub(crate) use derived::DERIVED_FORMAT;

mod legend;
mod ramp_catalog;
mod rgba;
mod themes;

pub use legend::{Legend, LegendStop};
pub(crate) use ramp_catalog::{index_fraction, ramp_colors};
pub use ramp_catalog::{palette_names, ramp_base_name, ramp_names};
pub use rgba::{Color, element_rgb};
pub use themes::{
    carbon_by_chain, chain, element, entity, metric, molecule_type, property, residue,
    residue_name, secondary_structure, uniform,
};

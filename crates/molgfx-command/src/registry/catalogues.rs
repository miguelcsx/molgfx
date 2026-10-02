//! The colour schemes, palettes and ramps by name.

/// Colour schemes computed from each atom: element, every category, carbon by
/// chain, and every metric.
#[must_use]
pub fn schemes() -> Vec<&'static str> {
    let mut words = vec!["element", crate::ir::CARBON_BY_CHAIN];
    words.extend(
        molgfx_scene::color::AtomCategory::ALL.map(molgfx_scene::color::AtomCategory::name),
    );
    words.extend(molgfx_scene::color::AtomMetric::ALL.map(molgfx_scene::color::AtomMetric::name));
    words.extend(molgfx_scene::color::AtomMetric::alias_names());
    words
}

/// Palettes a categorical colour can use.
#[must_use]
pub fn palettes() -> Vec<&'static str> {
    molgfx_scene::color::palette_names()
}

/// Ramps a property colour can use.
/// Every ramp name, each also usable reversed with an `_r` suffix.
#[must_use]
pub fn ramps() -> Vec<&'static str> {
    molgfx_scene::color::ramp_names()
}

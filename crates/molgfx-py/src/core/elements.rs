//! Element appearance by atomic number.
//!
//! A caller drawing atoms without a per-atom property column still needs a
//! radius and a colour per element, and both are pure functions of the atomic
//! number. The engine owns the tables so a Python caller never restates them.

use crate::math::PyRgba8;
use pyo3::prelude::*;

/// Radius of one element in ångström, falling back to hydrogen's.
#[pyfunction]
pub(crate) fn vdw_radius(atomic_number: u8) -> f32 {
    molgfx::core::vdw_radius(atomic_number)
}

/// CPK colour of one element, falling back to the unknown-element colour.
#[pyfunction]
pub(crate) fn cpk_color(atomic_number: u8) -> PyRgba8 {
    PyRgba8(molgfx::core::cpk_color(atomic_number))
}

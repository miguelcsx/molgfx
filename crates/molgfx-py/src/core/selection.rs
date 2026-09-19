//! Python adapters for declarative atom selections.

use crate::error::core;
use crate::math::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "PropertyComparison", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyPropertyComparison {
    Less,
    LessOrEqual,
    Equal,
    GreaterOrEqual,
    Greater,
}

impl From<PyPropertyComparison> for molgfx::core::PropertyComparison {
    fn from(value: PyPropertyComparison) -> Self {
        match value {
            PyPropertyComparison::Less => Self::Less,
            PyPropertyComparison::LessOrEqual => Self::LessOrEqual,
            PyPropertyComparison::Equal => Self::Equal,
            PyPropertyComparison::GreaterOrEqual => Self::GreaterOrEqual,
            PyPropertyComparison::Greater => Self::Greater,
        }
    }
}

#[pyclass(name = "SecondaryStructure", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PySecondaryStructure {
    Coil,
    Helix,
    Strand,
    Turn,
}

impl From<PySecondaryStructure> for molgfx::core::SecondaryStructure {
    fn from(value: PySecondaryStructure) -> Self {
        match value {
            PySecondaryStructure::Coil => Self::Coil,
            PySecondaryStructure::Helix => Self::Helix,
            PySecondaryStructure::Strand => Self::Strand,
            PySecondaryStructure::Turn => Self::Turn,
        }
    }
}

#[pyclass(name = "Select", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySelect(pub(crate) molgfx::core::Select);

#[pymethods]
impl PySelect {
    #[staticmethod]
    fn parse(source: &str) -> PyResult<Self> {
        core(source.parse()).map(Self)
    }
    #[staticmethod]
    fn all() -> Self {
        Self(molgfx::core::Select::all())
    }
    #[staticmethod]
    fn none() -> Self {
        Self(molgfx::core::Select::none())
    }
    #[staticmethod]
    fn polymer() -> Self {
        Self(molgfx::core::Select::polymer())
    }
    #[staticmethod]
    fn protein() -> Self {
        Self(molgfx::core::Select::protein())
    }
    #[staticmethod]
    fn nucleic() -> Self {
        Self(molgfx::core::Select::nucleic())
    }
    #[staticmethod]
    fn ligands() -> Self {
        Self(molgfx::core::Select::ligands())
    }
    #[staticmethod]
    fn water() -> Self {
        Self(molgfx::core::Select::water())
    }
    #[staticmethod]
    fn branched() -> Self {
        Self(molgfx::core::Select::branched())
    }

    #[staticmethod]
    fn chain(label: &str) -> PyResult<Self> {
        core(molgfx::core::Select::chain(label)).map(Self)
    }
    #[staticmethod]
    fn residue_name(name: &str) -> PyResult<Self> {
        core(molgfx::core::Select::residue_name(name)).map(Self)
    }
    #[staticmethod]
    fn atom_name(name: &str) -> PyResult<Self> {
        core(molgfx::core::Select::atom_name(name)).map(Self)
    }
    #[staticmethod]
    fn residue(number: i32) -> Self {
        Self(molgfx::core::Select::residue(number))
    }
    #[staticmethod]
    fn element(symbol: &str) -> PyResult<Self> {
        core(molgfx::core::Select::element(symbol)).map(Self)
    }
    #[staticmethod]
    fn secondary(value: PySecondaryStructure) -> Self {
        Self(molgfx::core::Select::secondary(value.into()))
    }
    #[staticmethod]
    fn helix() -> Self {
        Self(molgfx::core::Select::helix())
    }
    #[staticmethod]
    fn sheet() -> Self {
        Self(molgfx::core::Select::sheet())
    }
    #[staticmethod]
    fn coil() -> Self {
        Self(molgfx::core::Select::coil())
    }
    #[staticmethod]
    fn hydrogen() -> Self {
        Self(molgfx::core::Select::hydrogen())
    }
    #[staticmethod]
    fn heavy() -> Self {
        Self(molgfx::core::Select::heavy())
    }
    #[staticmethod]
    fn backbone() -> Self {
        Self(molgfx::core::Select::backbone())
    }
    #[staticmethod]
    fn terminus() -> Self {
        Self(molgfx::core::Select::terminus())
    }
    #[staticmethod]
    fn b_factor(comparison: PyPropertyComparison, threshold: f32) -> PyResult<Self> {
        core(molgfx::core::Select::b_factor(comparison.into(), threshold)).map(Self)
    }
    #[staticmethod]
    fn occupancy(comparison: PyPropertyComparison, threshold: f32) -> PyResult<Self> {
        core(molgfx::core::Select::occupancy(
            comparison.into(),
            threshold,
        ))
        .map(Self)
    }
    #[staticmethod]
    fn within(distance: f32, reference: PySelect) -> PyResult<Self> {
        core(molgfx::core::Select::within(distance, reference.0)).map(Self)
    }
    #[staticmethod]
    fn residues_within(distance: f32, reference: PySelect) -> PyResult<Self> {
        core(molgfx::core::Select::residues_within(distance, reference.0)).map(Self)
    }
    #[staticmethod]
    fn beyond(distance: f32, reference: PySelect) -> PyResult<Self> {
        core(molgfx::core::Select::beyond(distance, reference.0)).map(Self)
    }
    #[staticmethod]
    fn in_sphere(center: PyVec3, radius: f32) -> PyResult<Self> {
        core(molgfx::core::Select::in_sphere(center.0, radius)).map(Self)
    }
    #[staticmethod]
    fn in_box(min: PyVec3, max: PyVec3) -> PyResult<Self> {
        core(molgfx::core::Select::in_box(min.0, max.0)).map(Self)
    }

    // Trailing underscores: the alternative is exporting the Python keywords
    // `and` and `or`, which attribute access cannot reach.
    fn and_(&self, other: PySelect) -> Self {
        Self(self.0.clone().and(other.0))
    }
    fn or_(&self, other: PySelect) -> Self {
        Self(self.0.clone().or(other.0))
    }
    fn negate(&self) -> Self {
        Self(self.0.clone().negate())
    }
    fn __repr__(&self) -> String {
        "Select(...)".to_owned()
    }
}

#[pyfunction]
pub(crate) fn select(source: &str) -> PyResult<PySelect> {
    PySelect::parse(source)
}

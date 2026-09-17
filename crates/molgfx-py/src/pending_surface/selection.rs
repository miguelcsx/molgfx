//! Compact atom-selection adapter without Python-side set logic.

use pyo3::prelude::*;

#[pyclass(name = "AtomSelection", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAtomSelection(pub(crate) molgfx::AtomSelection);

#[pymethods]
impl PyAtomSelection {
    #[staticmethod]
    fn empty() -> Self {
        Self(molgfx::AtomSelection::Empty)
    }

    #[staticmethod]
    fn all() -> Self {
        Self(molgfx::AtomSelection::All)
    }

    #[staticmethod]
    fn range(start: u32, end: u32) -> Self {
        Self(molgfx::AtomSelection::Range(start..end))
    }

    #[staticmethod]
    fn copy_sparse(rows: Vec<u32>) -> Self {
        Self(molgfx::AtomSelection::Sparse(rows))
    }

    fn count(&self, table_len: u32) -> u64 {
        self.0.count(table_len)
    }

    fn contains(&self, row: u32) -> bool {
        self.0.contains(row)
    }

    fn union(&self, other: &Self, table_len: u32) -> Self {
        Self(self.0.union(&other.0, table_len))
    }

    fn intersect(&self, other: &Self, table_len: u32) -> Self {
        Self(self.0.intersect(&other.0, table_len))
    }

    fn difference(&self, other: &Self, table_len: u32) -> Self {
        Self(self.0.difference(&other.0, table_len))
    }

    fn copy_rows(&self, table_len: u32) -> Vec<u32> {
        let mut rows = Vec::new();
        self.0.for_each(table_len, |row| rows.push(row));
        rows
    }
}

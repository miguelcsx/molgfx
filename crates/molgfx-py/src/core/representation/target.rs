//! The scene data one representation visualizes.

use crate::core::{PySegmentationHandle, PySelectionHandle, PyVolumeHandle};
use pyo3::prelude::*;

#[pyclass(name = "RepresentationTarget", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyRepresentationTarget(pub(crate) molgfx::core::RepresentationTarget);

#[pymethods]
impl PyRepresentationTarget {
    /// Selection a molecular representation draws, absent for the other
    /// targets.
    #[getter]
    fn selection_handle(&self) -> Option<PySelectionHandle> {
        match self.0 {
            molgfx::core::RepresentationTarget::Selection(handle) => Some(handle.into()),
            _ => None,
        }
    }

    /// Scalar grid a volume representation draws, absent for the other
    /// targets.
    #[getter]
    fn volume_handle(&self) -> Option<PyVolumeHandle> {
        match self.0 {
            molgfx::core::RepresentationTarget::Volume(handle) => Some(handle.into()),
            _ => None,
        }
    }

    /// Categorical label grid a segmentation draws, absent for the other
    /// targets.
    #[getter]
    fn segmentation_handle(&self) -> Option<PySegmentationHandle> {
        match self.0 {
            molgfx::core::RepresentationTarget::SegmentedVolume(handle) => Some(handle.into()),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.0 {
            molgfx::core::RepresentationTarget::Selection(handle) => format!(
                "RepresentationTarget.selection(SelectionHandle({}, {}))",
                handle.row(),
                handle.generation()
            ),
            molgfx::core::RepresentationTarget::Volume(handle) => format!(
                "RepresentationTarget.volume(VolumeHandle({}, {}))",
                handle.row(),
                handle.generation()
            ),
            molgfx::core::RepresentationTarget::SegmentedVolume(handle) => format!(
                "RepresentationTarget.segmented_volume(SegmentationHandle({}, {}))",
                handle.row(),
                handle.generation()
            ),
        }
    }
}

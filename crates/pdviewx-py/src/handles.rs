//! Python adapters for generation-checked scene identities.

use pyo3::prelude::*;

macro_rules! handle_type {
    ($rust:ident, $python:literal, $native:ident) => {
        #[pyclass(name = $python, frozen, eq, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $rust(pub(crate) pdviewx::$native);

        #[pymethods]
        impl $rust {
            #[getter]
            fn row(&self) -> u32 {
                self.0.row()
            }
            #[getter]
            fn generation(&self) -> u32 {
                self.0.generation()
            }
            fn __repr__(&self) -> String {
                format!("{}({}, {})", $python, self.row(), self.generation())
            }
        }

        impl From<pdviewx::$native> for $rust {
            fn from(value: pdviewx::$native) -> Self {
                Self(value)
            }
        }
    };
}

handle_type!(PyStructureHandle, "StructureHandle", StructureHandle);
handle_type!(
    PyRepresentationHandle,
    "RepresentationHandle",
    RepresentationHandle
);
handle_type!(PySelectionHandle, "SelectionHandle", SelectionHandle);
handle_type!(PyVolumeHandle, "VolumeHandle", VolumeHandle);
handle_type!(
    PySegmentationHandle,
    "SegmentationHandle",
    SegmentationHandle
);
handle_type!(
    PyAtomPropertyHandle,
    "AtomPropertyHandle",
    AtomPropertyHandle
);
handle_type!(PyMeshHandle, "MeshHandle", MeshHandle);
handle_type!(
    PyMeshInstanceHandle,
    "MeshInstanceHandle",
    MeshInstanceHandle
);
handle_type!(PyOverlayHandle, "OverlayHandle", OverlayHandle);

macro_rules! opaque_handle_type {
    ($rust:ident, $python:literal, $native:ident) => {
        #[pyclass(name = $python, frozen, eq, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $rust(pub(crate) pdviewx::$native);

        #[pymethods]
        impl $rust {
            fn __repr__(&self) -> String {
                $python.to_owned()
            }
        }

        impl From<pdviewx::$native> for $rust {
            fn from(value: pdviewx::$native) -> Self {
                Self(value)
            }
        }
    };
}

opaque_handle_type!(PyEnsembleHandle, "EnsembleHandle", EnsembleHandle);
opaque_handle_type!(PyInteractionHandle, "InteractionHandle", InteractionHandle);
opaque_handle_type!(PyGuideHandle, "GuideHandle", GuideHandle);
opaque_handle_type!(PyAnnotationHandle, "AnnotationHandle", AnnotationHandle);
opaque_handle_type!(PyMeasurementHandle, "MeasurementHandle", MeasurementHandle);

#[pyclass(name = "EntityKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyEntityKind {
    Atom,
    Bond,
    Edge,
    Label,
    Primitive,
    Mesh,
}

impl From<PyEntityKind> for pdviewx::EntityKind {
    fn from(value: PyEntityKind) -> Self {
        match value {
            PyEntityKind::Atom => Self::Atom,
            PyEntityKind::Bond => Self::Bond,
            PyEntityKind::Edge => Self::Edge,
            PyEntityKind::Label => Self::Label,
            PyEntityKind::Primitive => Self::Primitive,
            PyEntityKind::Mesh => Self::Mesh,
        }
    }
}

impl From<pdviewx::EntityKind> for PyEntityKind {
    fn from(value: pdviewx::EntityKind) -> Self {
        match value {
            pdviewx::EntityKind::Atom => Self::Atom,
            pdviewx::EntityKind::Bond => Self::Bond,
            pdviewx::EntityKind::Edge => Self::Edge,
            pdviewx::EntityKind::Label => Self::Label,
            pdviewx::EntityKind::Primitive => Self::Primitive,
            pdviewx::EntityKind::Mesh => Self::Mesh,
        }
    }
}

#[pyclass(name = "EntityRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEntityRef(pub(crate) pdviewx::EntityRef);

impl From<pdviewx::EntityRef> for PyEntityRef {
    fn from(value: pdviewx::EntityRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyEntityRef {
    #[new]
    fn new(structure: PyStructureHandle, kind: PyEntityKind, index: u32) -> Self {
        Self(pdviewx::EntityRef {
            structure: structure.0,
            kind: kind.into(),
            index,
        })
    }
    #[getter]
    fn structure(&self) -> PyStructureHandle {
        self.0.structure.into()
    }
    #[getter]
    fn kind(&self) -> PyEntityKind {
        self.0.kind.into()
    }
    #[getter]
    fn index(&self) -> u32 {
        self.0.index
    }
}

#[pyclass(name = "VolumeSegmentRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVolumeSegmentRef(pub(crate) pdviewx::VolumeSegmentRef);

impl From<pdviewx::VolumeSegmentRef> for PyVolumeSegmentRef {
    fn from(value: pdviewx::VolumeSegmentRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyVolumeSegmentRef {
    #[new]
    fn new(volume: PySegmentationHandle, label: u32) -> Self {
        Self(pdviewx::VolumeSegmentRef {
            volume: volume.0,
            label,
        })
    }
    #[getter]
    fn volume(&self) -> PySegmentationHandle {
        self.0.volume.into()
    }
    #[getter]
    fn label(&self) -> u32 {
        self.0.label
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyStructureHandle>()?;
    module.add_class::<PyRepresentationHandle>()?;
    module.add_class::<PySelectionHandle>()?;
    module.add_class::<PyVolumeHandle>()?;
    module.add_class::<PySegmentationHandle>()?;
    module.add_class::<PyAtomPropertyHandle>()?;
    module.add_class::<PyMeshHandle>()?;
    module.add_class::<PyMeshInstanceHandle>()?;
    module.add_class::<PyOverlayHandle>()?;
    module.add_class::<PyEnsembleHandle>()?;
    module.add_class::<PyInteractionHandle>()?;
    module.add_class::<PyGuideHandle>()?;
    module.add_class::<PyAnnotationHandle>()?;
    module.add_class::<PyMeasurementHandle>()?;
    module.add_class::<PyEntityKind>()?;
    module.add_class::<PyEntityRef>()?;
    module.add_class::<PyVolumeSegmentRef>()
}

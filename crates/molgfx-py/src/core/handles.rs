//! Python adapters for generation-checked scene identities.

use pyo3::prelude::*;

macro_rules! handle_type {
    ($rust:ident, $python:literal, $native:ident) => {
        #[pyclass(name = $python, frozen, eq, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $rust(pub(crate) molgfx::core::$native);

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

        impl From<molgfx::core::$native> for $rust {
            fn from(value: molgfx::core::$native) -> Self {
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
handle_type!(PyEnsembleHandle, "EnsembleHandle", EnsembleHandle);
handle_type!(
    PyAtomPropertyHandle,
    "AtomPropertyHandle",
    AtomPropertyHandle
);
handle_type!(
    PyLigandPoseBatchHandle,
    "LigandPoseBatchHandle",
    LigandPoseBatchHandle
);
handle_type!(PyMeshHandle, "MeshHandle", MeshHandle);
handle_type!(
    PyMeshInstanceHandle,
    "MeshInstanceHandle",
    MeshInstanceHandle
);
handle_type!(PyOverlayHandle, "OverlayHandle", OverlayHandle);
handle_type!(PyPrimitiveHandle, "PrimitiveHandle", PrimitiveHandle);
handle_type!(PyAttributeHandle, "AttributeHandle", AttributeHandle);
handle_type!(PyPointBatchHandle, "PointBatchHandle", PointBatchHandle);
handle_type!(
    PyInstanceBatchHandle,
    "InstanceBatchHandle",
    InstanceBatchHandle
);
handle_type!(
    PyRelationBatchHandle,
    "RelationBatchHandle",
    RelationBatchHandle
);
handle_type!(
    PyTimelineTrackHandle,
    "TimelineTrackHandle",
    TimelineTrackHandle
);

macro_rules! opaque_handle_type {
    ($rust:ident, $python:literal, $native:ident) => {
        #[pyclass(name = $python, frozen, eq, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $rust(pub(crate) molgfx::core::$native);

        #[pymethods]
        impl $rust {
            fn __repr__(&self) -> String {
                $python.to_owned()
            }
        }

        impl From<molgfx::core::$native> for $rust {
            fn from(value: molgfx::core::$native) -> Self {
                Self(value)
            }
        }
    };
}

opaque_handle_type!(PyGuideHandle, "GuideHandle", GuideHandle);
opaque_handle_type!(PyInteractionHandle, "InteractionHandle", InteractionHandle);
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
    Guide,
    DynamicBond,
    Point,
    Instance,
    TemplatePart,
    Relation,
    LigandPoseBatch,
}

impl From<PyEntityKind> for molgfx::core::EntityKind {
    fn from(value: PyEntityKind) -> Self {
        match value {
            PyEntityKind::Atom => Self::Atom,
            PyEntityKind::Bond => Self::Bond,
            PyEntityKind::Edge => Self::Edge,
            PyEntityKind::Label => Self::Label,
            PyEntityKind::Primitive => Self::Primitive,
            PyEntityKind::Mesh => Self::Mesh,
            PyEntityKind::Guide => Self::Guide,
            PyEntityKind::DynamicBond => Self::DynamicBond,
            PyEntityKind::Point => Self::Point,
            PyEntityKind::Instance => Self::Instance,
            PyEntityKind::TemplatePart => Self::TemplatePart,
            PyEntityKind::Relation => Self::Relation,
            PyEntityKind::LigandPoseBatch => Self::LigandPoseBatch,
        }
    }
}

impl From<molgfx::core::EntityKind> for PyEntityKind {
    fn from(value: molgfx::core::EntityKind) -> Self {
        match value {
            molgfx::core::EntityKind::Atom => Self::Atom,
            molgfx::core::EntityKind::Bond => Self::Bond,
            molgfx::core::EntityKind::Edge => Self::Edge,
            molgfx::core::EntityKind::Label => Self::Label,
            molgfx::core::EntityKind::Primitive => Self::Primitive,
            molgfx::core::EntityKind::Mesh => Self::Mesh,
            molgfx::core::EntityKind::Guide => Self::Guide,
            molgfx::core::EntityKind::DynamicBond => Self::DynamicBond,
            molgfx::core::EntityKind::Point => Self::Point,
            molgfx::core::EntityKind::Instance => Self::Instance,
            molgfx::core::EntityKind::TemplatePart => Self::TemplatePart,
            molgfx::core::EntityKind::Relation => Self::Relation,
            molgfx::core::EntityKind::LigandPoseBatch => Self::LigandPoseBatch,
        }
    }
}

#[pyclass(name = "EntityRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEntityRef(pub(crate) molgfx::core::EntityRef);

impl From<molgfx::core::EntityRef> for PyEntityRef {
    fn from(value: molgfx::core::EntityRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyEntityRef {
    #[new]
    fn new(structure: PyStructureHandle, kind: PyEntityKind, index: u32) -> Self {
        Self(molgfx::core::EntityRef {
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
pub(crate) struct PyVolumeSegmentRef(pub(crate) molgfx::core::VolumeSegmentRef);

impl From<molgfx::core::VolumeSegmentRef> for PyVolumeSegmentRef {
    fn from(value: molgfx::core::VolumeSegmentRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyVolumeSegmentRef {
    #[new]
    fn new(volume: PySegmentationHandle, label: u32) -> Self {
        Self(molgfx::core::VolumeSegmentRef {
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

//! Python adapters for generation-checked scene identities.

use pyo3::prelude::*;

macro_rules! handle_type {
    ($rust:ident, $python:literal, $native:ident) => {
        #[pyclass(name = $python, frozen, eq, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) struct $rust(pub(crate) molgfx::$native);

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

        impl From<molgfx::$native> for $rust {
            fn from(value: molgfx::$native) -> Self {
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
        pub(crate) struct $rust(pub(crate) molgfx::$native);

        #[pymethods]
        impl $rust {
            fn __repr__(&self) -> String {
                $python.to_owned()
            }
        }

        impl From<molgfx::$native> for $rust {
            fn from(value: molgfx::$native) -> Self {
                Self(value)
            }
        }
    };
}

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
    Guide,
    DynamicBond,
    Point,
    Instance,
    TemplatePart,
    Relation,
}

impl From<PyEntityKind> for molgfx::EntityKind {
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
        }
    }
}

#[pyclass(name = "EntityRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEntityRef(pub(crate) molgfx::EntityRef);

impl From<molgfx::EntityRef> for PyEntityRef {
    fn from(value: molgfx::EntityRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyEntityRef {
    #[new]
    fn new(structure: PyStructureHandle, kind: PyEntityKind, index: u32) -> Self {
        Self(molgfx::EntityRef {
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
    fn kind(&self) -> PyResult<PyEntityKind> {
        match self.0.kind {
            molgfx::EntityKind::Atom => Ok(PyEntityKind::Atom),
            molgfx::EntityKind::Bond => Ok(PyEntityKind::Bond),
            molgfx::EntityKind::Edge => Ok(PyEntityKind::Edge),
            molgfx::EntityKind::Label => Ok(PyEntityKind::Label),
            molgfx::EntityKind::Primitive => Ok(PyEntityKind::Primitive),
            molgfx::EntityKind::Mesh => Ok(PyEntityKind::Mesh),
            molgfx::EntityKind::Guide => Ok(PyEntityKind::Guide),
            molgfx::EntityKind::DynamicBond => Ok(PyEntityKind::DynamicBond),
            molgfx::EntityKind::Point => Ok(PyEntityKind::Point),
            molgfx::EntityKind::Instance => Ok(PyEntityKind::Instance),
            molgfx::EntityKind::TemplatePart => Ok(PyEntityKind::TemplatePart),
            molgfx::EntityKind::Relation => Ok(PyEntityKind::Relation),
            molgfx::EntityKind::LigandPoseBatch => Err(crate::error::value(
                "legacy ligand-pose entities are not part of the public API",
            )),
        }
    }
    #[getter]
    fn index(&self) -> u32 {
        self.0.index
    }
}

#[pyclass(name = "VolumeSegmentRef", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVolumeSegmentRef(pub(crate) molgfx::VolumeSegmentRef);

impl From<molgfx::VolumeSegmentRef> for PyVolumeSegmentRef {
    fn from(value: molgfx::VolumeSegmentRef) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyVolumeSegmentRef {
    #[new]
    fn new(volume: PySegmentationHandle, label: u32) -> Self {
        Self(molgfx::VolumeSegmentRef {
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
    module.add_class::<PyPrimitiveHandle>()?;
    module.add_class::<PyAttributeHandle>()?;
    module.add_class::<PyPointBatchHandle>()?;
    module.add_class::<PyInstanceBatchHandle>()?;
    module.add_class::<PyRelationBatchHandle>()?;
    module.add_class::<PyTimelineTrackHandle>()?;
    module.add_class::<PyGuideHandle>()?;
    module.add_class::<PyAnnotationHandle>()?;
    module.add_class::<PyMeasurementHandle>()?;
    module.add_class::<PyEntityKind>()?;
    module.add_class::<PyEntityRef>()?;
    module.add_class::<PyVolumeSegmentRef>()
}

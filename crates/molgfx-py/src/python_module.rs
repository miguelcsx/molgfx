//! The native module: the whole Python surface, declared once.
//!
//! Nothing here is registered by hand. Each `#[pymodule_export]` names a Rust
//! item, and the export is derived from that item's own declaration — its
//! Python name, its module and its signature. A binding that stops being
//! exported stops compiling, so the module cannot silently drift from the
//! facade.
//!
//! The tree mirrors the facade: `molgfx._engine.core` corresponds to
//! `molgfx::core`, `molgfx._engine.render` to `molgfx::render`, and so
//! on, so a name is reachable from Python in the same place a Rust caller
//! would find it. The exceptions sit at the root because they describe the
//! module as a whole rather than any one subsystem.

use pyo3::create_exception;
use pyo3::prelude::*;

create_exception!(
    _engine,
    AttributeRemovedError,
    pyo3::exceptions::PyAttributeError,
    "Raised for an attribute the package does not define, so a lookup that\n\
     names the wrong namespace says which namespaces hold the surface."
);

#[pymodule]
mod _engine {
    use pyo3::prelude::*;
    use pyo3::types::PyType;

    #[pymodule_export]
    use crate::error::{
        ChunkPlacementError, ChunkResidencyError, CoreError, GpuError, ManifestError, MolgfxError,
        RenderError, SemanticError, VisualError,
    };
    #[pymodule_export]
    use crate::pending_surface::mapping::MappingError;
    #[pymodule_export]
    use crate::python_module::AttributeRemovedError;

    /// Adds the version the package reports as `molgfx.__version__`, gives each
    /// submodule its dotted name, and points every exported class at the module
    /// a caller imports it from.
    ///
    /// `PyO3` adds a nested module as an attribute of its parent and stops there,
    /// so without this a submodule answers to attribute access and to nothing
    /// else — `import molgfx._engine.core` cannot resolve a module that
    /// `sys.modules` has never heard of. The classes need naming too: a
    /// `#[pyclass]` that declares no module of its own reports `builtins`, which
    /// is what `repr` shows and what `pickle` would try to import.
    #[pymodule_init]
    fn init(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add("__version__", env!("CARGO_PKG_VERSION"))?;
        let modules = module.py().import("sys")?.getattr("modules")?;
        let native = module.name()?.to_str()?.to_owned();
        let package = native
            .rsplit_once('.')
            .map(|(package, _)| package.to_owned());
        for name in ["core", "math", "render", "semantic"] {
            let submodule = module.getattr(name)?;
            let dotted = format!("{native}.{name}");
            submodule.setattr("__name__", &dotted)?;
            modules.set_item(&dotted, &submodule)?;
            let home = match &package {
                Some(package) => format!("{package}.{name}"),
                None => dotted,
            };
            name_exports(&submodule, &home)?;
        }
        if let Some(package) = &package {
            name_exports(module.as_any(), package)?;
        }
        Ok(())
    }

    /// Names every class a module exports as living in `home`.
    fn name_exports(module: &Bound<'_, PyAny>, home: &str) -> PyResult<()> {
        for export in module.getattr("__all__")?.try_iter()? {
            let export = export?;
            let name: &str = export.extract()?;
            let item = module.getattr(name)?;
            if item.is_instance_of::<PyType>() {
                item.setattr("__module__", home)?;
            }
        }
        Ok(())
    }

    /// Scene graph, selections, representations, values and authoring.
    ///
    /// This is the layer a program spends its time in: the structures,
    /// selections, representations and columns a caller builds and edits,
    /// plus the handles that name them. Constants that bound a column live
    /// beside the types they bound.
    #[pymodule]
    mod core {
        #[pymodule_export]
        use crate::annotations::{
            PyAnnotation, PyAnnotationAnchor, PyAnnotationKind, PyMarkerShape, PyMarkerStyle,
            PyMeasurement, PyMeasurementKind,
        };
        #[pymodule_export]
        use crate::authoring::{PyGuideCap, PyGuideStyle, PyMeshTopology, PyParticleShape};
        #[pymodule_export]
        use crate::controls::{
            PyArcballController, PyButton, PyFlyController, PyInputEvent, PyKey, PyOrbitController,
        };
        #[pymodule_export]
        use crate::core::atom_property::{PyAtomProperty, PyAtomPropertyMeaning};
        #[pymodule_export]
        use crate::core::elements::{cpk_color, vdw_radius};
        #[pymodule_export]
        use crate::core::ensemble::PyEnsemble;
        #[pymodule_export]
        use crate::core::generic_batches::anchors::{
            PyAnchorLayout, PyRelationLayout, PyRowEntityRef, PySpatialAnchor, PyTemplatePartRef,
        };
        #[pymodule_export]
        use crate::core::generic_batches::attributes::{
            PyAttributeColumn, PyAttributeDescriptor, PyAttributeKind, PyAttributeValues,
        };
        #[pymodule_export]
        use crate::core::generic_batches::instances::{
            PyInstanceBatch, PyInstanceStyle, PyRigidInstance,
        };
        #[pymodule_export]
        use crate::core::generic_batches::model::{PyRelationPattern, PyRowDomain};
        #[pymodule_export]
        use crate::core::generic_batches::points::{PyPointBatch, PyPointGlyph, PyPointStyle};
        #[pymodule_export]
        use crate::core::generic_batches::primitives::{PyParticle, PyPrimitive};
        #[pymodule_export]
        use crate::core::generic_batches::relations::{
            PyRelation, PyRelationBatch, PyRelationDependency, PyRelationPartition,
        };
        #[pymodule_export]
        use crate::core::generic_batches::template::{
            PyAnalyticCapsule, PyAnalyticSphere, PyAnalyticTemplate,
        };
        #[pymodule_export]
        use crate::core::handles::{
            PyAnnotationHandle, PyAtomPropertyHandle, PyAttributeHandle, PyEnsembleHandle,
            PyEntityKind, PyEntityRef, PyGuideHandle, PyInstanceBatchHandle, PyInteractionHandle,
            PyLigandPoseBatchHandle, PyMeasurementHandle, PyMeshHandle, PyMeshInstanceHandle,
            PyOverlayHandle, PyPointBatchHandle, PyPrimitiveHandle, PyRelationBatchHandle,
            PyRepresentationHandle, PySegmentationHandle, PySelectionHandle, PyStructureHandle,
            PyTimelineTrackHandle, PyVolumeHandle, PyVolumeSegmentRef,
        };
        #[pymodule_export]
        use crate::core::interaction::{
            PyInteraction, PyInteractionAnchor, PyInteractionDirection, PyInteractionGeometry,
            PyInteractionKind, PyInteractionPattern, PyInteractionStyle,
        };
        #[pymodule_export]
        use crate::core::ligand_pose::{PyLicoriceTemplate, PyLigandPose, PyLigandPoseBatch};
        #[pymodule_export]
        use crate::core::provenance::PyProvenance;
        #[pymodule_export]
        use crate::core::representation::{
            PyPropertyAppearance, PyPropertyAppearanceSample, PyRelationStyle, PyRepresentation,
            PyRepresentationKind, PyRepresentationPreset, PyRepresentationTarget,
        };
        #[pymodule_export]
        use crate::core::scene::PyScene;
        #[pymodule_export]
        use crate::core::selection::{PyPropertyComparison, PySecondaryStructure, PySelect};
        #[pymodule_export]
        use crate::core::serialization::{
            PyAnchorDescription, PyAnnotationDescription, PyAtomPropertyDescription,
            PyAttributeDescription, PyClipDescription, PyColorDescription, PyContentAddress,
            PyDomainVisualDescription, PyEntityDescription, PyGuideDescription,
            PyGuideStyleDescription, PyInstanceBatchDescription, PyInteractionDescription,
            PyLigandPoseBatchDescription, PyLigandPoseDescription, PyMarkerStyleDescription,
            PyMaterialDescription, PyMeasurementDescription, PyObjectIdentity,
            PyOccupancyDescription, PyPayloadReference, PyPointBatchDescription,
            PyPropertyAppearanceDescription, PyReferencedPayloadKind, PyRegionDescription,
            PyRelationBatchDescription, PyRepresentationDescription, PyRowDomainDescription,
            PyScalarSemanticsDescription, PySceneDescription, PySceneManifest,
            PySegmentStyleDescription, PySegmentationStyleDescription, PySelectionDescription,
            PySelectionMask, PySourceRowsDescription, PyStructureDescription,
            PySurfaceComponentDescription, PySurfaceScalarDescription, PyTableCounts,
            PyTargetDescription, PyVisualAttributeDescription, PyVisualInstructionDescription,
            PyVisualStyleDescription, PyVolumeDescription, PyVolumeStyleDescription,
            PyVolumeTransferPointDescription,
        };
        #[pymodule_export]
        use crate::core::serialization::{read_manifest, write_manifest};
        #[pymodule_export]
        use crate::core::volumes::{
            PyMaterial, PyMaterialModel, PyOccupancyStream, PyScalarVolume, PySegmentedVolume,
        };
        #[pymodule_export]
        use crate::memory::{PyMemoryOwnership, PyMemoryTransferExclusion};
        #[pymodule_export]
        use crate::mesh_instances::PyMeshInstance;
        #[pymodule_export]
        use crate::meshes::{PyMesh, PyMeshVertex};
        #[pymodule_export]
        use crate::overlay_authoring::PyOverlayAnchor;
        #[pymodule_export]
        use crate::pending_surface::descriptions::{
            PyMeshDescription, PyMeshInstanceDescription, PyOverlayDescription,
            PyParticleMotionDescription, PyPrimitiveDescription,
        };
        #[pymodule_export]
        use crate::pending_surface::geometry::{
            PyAnisotropicEllipsoid, PyCarbohydrateShape, PyCarbohydrateSymbol, PyGuide,
            PyPlanarRegion, PyQuadric,
        };
        #[pymodule_export]
        use crate::pending_surface::overlay::{PyOverlayContent, PyOverlayKind, PyScreenOverlay};
        #[pymodule_export]
        use crate::pending_surface::particle::{PyParticleBoundary, PyParticleMotion};
        #[pymodule_export]
        use crate::pending_surface::presentation::{
            PyFaceVisibility, PyPropertyLegend, PySurfaceComponentPolicy,
            PySurfaceComponentThreshold,
        };
        #[pymodule_export]
        use crate::pending_surface::provenance::{PyEntityProvenance, PyProvenanceDetail};
        #[pymodule_export]
        use crate::pending_surface::representation::{
            PyRepresentationConfig, PyRepresentationInput, PyRepresentationParams,
        };
        #[pymodule_export]
        use crate::pending_surface::selection::PyAtomSelection;
        #[pymodule_export]
        use crate::pending_surface::visual::PyVisualInstructionGpu;
        #[pymodule_export]
        use crate::pending_surface::volume::{
            PyVolumeRendering, PyVolumeTransferFunction, PyVolumeTransferPoint,
        };
        #[pymodule_export]
        use crate::timeline::{
            PyCameraBookmark, PyCameraEasing, PyCameraKeyframe, PyCameraPath, PyPlaybackMode,
            PyTimeWarp, PyTimeline,
        };
        #[pymodule_export]
        use crate::topology::{PyBondTopologyFrame, PyBondTopologySegment, PyTopologyBond};
        #[pymodule_export]
        use crate::trajectory::{
            PyTrajectoryBranch, PyTrajectoryChunkWindow, PyTrajectoryFrame, PyTrajectorySegment,
            PyTrajectoryStateGraph,
        };
        #[pymodule_export]
        use crate::validation_markers::{PyValidationKind, PyValidationMarker};
        #[pymodule_export]
        use crate::values::color::{
            PyColorScheme, PyScalarContours, PyScalarFieldSemantics, PyScalarRamp, PySurfaceKind,
            PySurfaceScalarOverlay, PySurfaceStyle,
        };
        #[pymodule_export]
        use crate::values::volume::{
            PyClipCap, PyClipPlane, PyClipSet, PyCrystalCell, PySegmentStyle, PySegmentStyleTable,
            PySegmentationStyle, PySymmetryInstance, PyTubeRadiusMapping, PyVolumeStyle,
        };
        #[pymodule_export]
        use crate::visual::compatibility::PyVisualCompatibility;
        #[pymodule_export]
        use crate::visual::enums::{PyVisualOutput, PyVisualStage};
        #[pymodule_export]
        use crate::visual::expressions::{
            PyBoolExpr, PyColorExpr, PyColorParameter, PyScalarExpr, PyScalarParameter,
            PyVectorExpr, PyVectorParameter,
        };
        #[pymodule_export]
        use crate::visual::{
            PyVisualAttributeRef, PyVisualColumnKey, PyVisualDescriptor, PyVisualEvaluation,
            PyVisualInputs, PyVisualProgram, PyVisualProgramBuilder, PyVisualStyle,
        };

        #[pymodule_export]
        use crate::core::selection::select;

        /// The most clip planes a clip set can hold.
        #[pymodule_export]
        const MAX_CLIP_PLANES: usize = molgfx::core::MAX_CLIP_PLANES;

        /// The most vertices a caller-supplied mesh can hold.
        #[pymodule_export]
        const MAX_MESH_VERTICES: usize = molgfx::core::MAX_MESH_VERTICES;

        /// The most instructions a visual program can hold.
        #[pymodule_export]
        const MAX_VISUAL_INSTRUCTIONS: usize = molgfx::core::MAX_VISUAL_INSTRUCTIONS;

        /// The most parameters a visual program can hold.
        #[pymodule_export]
        const MAX_VISUAL_PARAMETERS: usize = molgfx::core::MAX_VISUAL_PARAMETERS;

        /// The most properties a visual program can read at once.
        #[pymodule_export]
        const MAX_VISUAL_PROPERTIES: usize = molgfx::core::MAX_VISUAL_PROPERTIES;

        /// The most control points a volume transfer function can hold.
        #[pymodule_export]
        const MAX_VOLUME_TRANSFER_POINTS: usize = molgfx::core::MAX_VOLUME_TRANSFER_POINTS;
    }

    /// Project-owned math identities: vectors, quaternions, matrices, bounds
    /// and the camera.
    #[pymodule]
    mod math {
        #[pymodule_export]
        use crate::math::{
            PyAabb, PyBoundingSphere, PyCamera, PyCurveSample, PyMat3, PyMat4, PyProjection,
            PyQuat, PyRgba8, PyTransportFrame, PyVec2, PyVec3, PyVec4,
        };

        #[pymodule_export]
        use crate::math::{
            parallel_transport, round_u8, sample_catmull_rom, sample_catmull_rom_demanding,
            sample_catmull_rom_fixed, sample_cubic_bezier, truncate_u16, unit_to_grid, unorm8,
        };
    }

    /// The renderer: the engine, its configuration, and the chunk and brick
    /// residency that feeds it.
    #[pymodule]
    mod render {
        #[pymodule_export]
        use crate::render::brick::{
            PyBrickAddress, PyBrickCatalog, PyBrickDescriptor, PyBrickId, PyBrickMetadata,
            PyBrickShape, PyBrickValueRange, PyDirtyGeneration,
        };
        #[pymodule_export]
        use crate::render::brick_atlas::{
            PyBrickAtlasConfig, PyBrickAtlasKind, PyBrickAtlasMetrics, PyFenceValue,
            PyUploadRingConfig,
        };
        #[pymodule_export]
        use crate::render::chunk_placement::{
            PyAttributeChunkWindow, PyBondChunkPlacement, PyChunkPlacementId,
            PyChunkRepresentation, PyInstanceChunkWindow, PyRelationChunkPlacement,
            PyStructureChunkPlacement,
        };
        #[pymodule_export]
        use crate::render::chunk_residency::{
            PyArenaMetrics, PyChunkResidencyMetrics, PyDerivedCacheUsage, PyFrameCompleteness,
            PyFrameDegradation, PyFrameMetrics, PyResidentStructureChunk,
            PyResidentTrajectoryChunk, PyUploadMetrics,
        };
        #[pymodule_export]
        use crate::render::chunk_streaming::{
            PyChunkPlacementStatus, PyInstanceChunkPlacement, PyPointChunkPlacement,
            PyResidentGenericChunk,
        };
        #[pymodule_export]
        use crate::render::config::{
            PyCapabilities, PyDerivedCacheBudget, PyEngineConfig, PyFrameReport, PyFrameStatus,
            PyFrameTicket, PyImageConfig, PyPowerPreference, PyRenderMode, PySequenceConfig,
        };
        #[pymodule_export]
        use crate::render::engine::{
            PyEngine, PyFrameTiming, PyGlobalPickIdentity, PyHdrImage, PyImage, PyPick,
            PyPickEntity,
        };
        #[pymodule_export]
        use crate::render::engine_sequence::{PySequenceFrame, PySequenceRenderer};
        #[pymodule_export]
        use crate::render::profile::composition::{PyRenderProfile, PyResolvedRenderPlan};
        #[pymodule_export]
        use crate::render::profile::context::{
            PyBackdropStyle, PyIllustrationStyle, PyLightingEnvironment,
        };
        #[pymodule_export]
        use crate::render::profile::display::{
            PyDisplayGamut, PyDisplayTransform, PyToneMapping, PyTransferFunction,
        };
        #[pymodule_export]
        use crate::render::profile::effects::{
            PyBloomStyle, PyDepthOfField, PyEffectLayer, PyMotionBlur, PyPresentationEffect,
        };
        #[pymodule_export]
        use crate::render::profile::focus::PyFocusTarget;
        #[pymodule_export]
        use crate::render::session::PyRenderSession;
    }

    /// The semantic layer: focus and context, level of detail, streaming
    /// policy and property mapping.
    #[pymodule]
    mod semantic {
        #[pymodule_export]
        use crate::pending_surface::mapping::PyPropertyMapping;
        #[pymodule_export]
        use crate::pending_surface::semantic::{
            PyLodCluster, PySurfaceZone, PySurfaceZoneScene, PySurfaceZoneStyle,
        };
        #[pymodule_export]
        use crate::semantic::composition::{
            PyDifferenceCompositionStyle, PyDifferenceLayer, PyDistanceBands,
            PyEnsembleCompositionStyle, PyEnsembleLayer, PyFocusBand, PyFocusCompositionStyle,
            PyFocusContext, PyFocusLayer, PyFocusScene, PyFocusStyle, PyFocusSurfaceExtent,
            PyFocusView, PyGenericCompositionScene,
        };
        #[pymodule_export]
        use crate::semantic::dataset::{
            PyChunkBounds, PyChunkDescriptor, PyChunkFootprint, PyChunkId, PyChunkSpan,
            PyDatasetCatalog, PyDatasetId, PyLocalRow, PyLogicalRow, PyPayloadKind,
        };
        #[pymodule_export]
        use crate::semantic::generic::model::PyGenericCompositionView;
        #[pymodule_export]
        use crate::semantic::residency::{
            PyDeviceLossReport, PyResidencyBudget, PyResidencyClass, PyResidencyEviction,
            PyResidencyFailure, PyResidencyKey, PyResidencyMachine, PyResidencyOutput,
            PyResidencyPhase, PyResidencyRequest, PyResidencySnapshot, PyResidencyTicket,
            PyResidencyUsage, PyStaleCompletion,
        };
        #[pymodule_export]
        use crate::semantic::streaming::{
            PyChunkKey, PyChunkRequest, PyLodClusterKey, PyLodFrame, PyLodIndex, PyLodLevel,
            PyLodPolicy, PyLodScene, PyStreamPlan, PyStreamPlanner, PyStreamingBudget,
        };
    }
}

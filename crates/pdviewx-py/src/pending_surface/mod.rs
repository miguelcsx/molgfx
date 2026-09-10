//! Internal typed adapters for the facade surface-completion slice.
//!
//! Registration is generated from the canonical binding specification; these
//! modules only translate Python values to the Rust-owned facade contracts.

mod descriptions;
mod geometry;
mod mapping;
mod overlay;
mod particle;
mod presentation;
mod provenance;
mod representation;
mod selection;
mod semantic;
mod visual;
mod volume;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<geometry::PyAnisotropicEllipsoid>()?;
    module.add_class::<selection::PyAtomSelection>()?;
    module.add_class::<geometry::PyCarbohydrateShape>()?;
    module.add_class::<geometry::PyCarbohydrateSymbol>()?;
    module.add_class::<provenance::PyEntityProvenance>()?;
    module.add_class::<presentation::PyFaceVisibility>()?;
    module.add_class::<geometry::PyGuide>()?;
    module.add_class::<semantic::PyLodCluster>()?;
    module.add(
        "MappingError",
        module.py().get_type::<mapping::MappingError>(),
    )?;
    module.add_class::<descriptions::PyMeshDescription>()?;
    module.add_class::<descriptions::PyMeshInstanceDescription>()?;
    module.add_class::<descriptions::PyOverlayDescription>()?;
    module.add_class::<particle::PyParticleBoundary>()?;
    module.add_class::<particle::PyParticleMotion>()?;
    module.add_class::<descriptions::PyParticleMotionDescription>()?;
    module.add_class::<geometry::PyPlanarRegion>()?;
    module.add_class::<descriptions::PyPrimitiveDescription>()?;
    module.add_class::<presentation::PyPropertyLegend>()?;
    module.add_class::<mapping::PyPropertyMapping>()?;
    module.add_class::<provenance::PyProvenanceDetail>()?;
    module.add_class::<geometry::PyQuadric>()?;
    module.add_class::<representation::PyRepresentationConfig>()?;
    module.add_class::<representation::PyRepresentationInput>()?;
    module.add_class::<representation::PyRepresentationParams>()?;
    module.add_class::<overlay::PyScreenOverlay>()?;
    module.add_class::<presentation::PySurfaceComponentPolicy>()?;
    module.add_class::<semantic::PySurfaceZone>()?;
    module.add_class::<semantic::PySurfaceZoneScene>()?;
    module.add_class::<semantic::PySurfaceZoneStyle>()?;
    module.add_class::<visual::PyVisualInstructionGpu>()?;
    module.add_class::<volume::PyVolumeRendering>()?;
    module.add_class::<volume::PyVolumeTransferFunction>()?;
    module.add_class::<volume::PyVolumeTransferPoint>()
}

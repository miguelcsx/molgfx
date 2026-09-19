//! Reusable licorice topology and the rigid ligand candidates posed over it.
//!
//! One template describes the ligand's atoms and bonds once; each candidate is
//! one rigid pose over that topology, so a readout of a thousand docked
//! candidates uploads a thousand small records rather than a thousand copies
//! of the same molecule.

use crate::error::core;
use crate::math::{PyQuat, PyRgba8, PyVec3};
use pyo3::prelude::*;

/// Atom and bond geometry shared by every rigid candidate pose.
#[pyclass(name = "LicoriceTemplate", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyLicoriceTemplate(pub(crate) molgfx::core::LicoriceTemplate);

impl From<&molgfx::core::LicoriceTemplate> for PyLicoriceTemplate {
    fn from(value: &molgfx::core::LicoriceTemplate) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyLicoriceTemplate {
    /// Builds a topology from atom positions and zero-based bond endpoints.
    ///
    /// Positions are stored relative to `origin`, which is where a pose later
    /// places the ligand.
    #[new]
    fn new(origin: PyVec3, atoms: Vec<PyVec3>, bonds: Vec<(u32, u32)>) -> PyResult<Self> {
        let atoms = atoms.into_iter().map(|atom| atom.0).collect::<Vec<_>>();
        let bonds = bonds
            .into_iter()
            .map(|(start, end)| [start, end])
            .collect::<Vec<_>>();
        core(molgfx::core::LicoriceTemplate::new(origin.0, atoms, &bonds)).map(Self)
    }

    /// Returns a copy drawing atoms and bonds at the given radii in ångström.
    fn with_radii(&self, atom_radius: f32, bond_radius: f32) -> PyResult<Self> {
        core(self.0.clone().radii(atom_radius, bond_radius)).map(Self)
    }

    /// Analytic instances one pose expands to.
    #[getter]
    fn instances_per_pose(&self) -> usize {
        self.0.instances_per_pose()
    }

    #[getter]
    fn atom_radius(&self) -> f32 {
        self.0.atom_radius()
    }

    #[getter]
    fn bond_radius(&self) -> f32 {
        self.0.bond_radius()
    }
}

/// One rigid occurrence of a reusable ligand template.
#[pyclass(name = "LigandPose", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyLigandPose(pub(crate) molgfx::core::LigandPose);

#[pymethods]
impl PyLigandPose {
    #[new]
    fn new(
        translation: PyVec3,
        orientation: PyQuat,
        color: PyRgba8,
        opacity: f32,
    ) -> PyResult<Self> {
        core(molgfx::core::LigandPose::new(
            translation.0,
            orientation.0,
            color.0,
            opacity,
        ))
        .map(Self)
    }

    /// Where the template origin lands.
    #[getter]
    fn translation(&self) -> PyVec3 {
        PyVec3(self.0.translation())
    }

    /// Unit rotation applied before the translation.
    #[getter]
    fn orientation(&self) -> PyQuat {
        PyQuat(self.0.orientation())
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color())
    }

    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity()
    }
}

/// Scene-owned batch of rigid candidates over one shared topology.
#[pyclass(name = "LigandPoseBatch", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyLigandPoseBatch {
    owner: molgfx::core::StructureHandle,
    template: molgfx::core::LicoriceTemplate,
    poses: Box<[molgfx::core::LigandPose]>,
    visible: bool,
}

impl From<&molgfx::core::LigandPoseBatch> for PyLigandPoseBatch {
    fn from(value: &molgfx::core::LigandPoseBatch) -> Self {
        Self {
            owner: value.owner(),
            template: value.template().clone(),
            poses: value.poses().into(),
            visible: value.visible(),
        }
    }
}

#[pymethods]
impl PyLigandPoseBatch {
    /// Structure carrying this batch's model-space transforms.
    #[getter]
    fn owner(&self) -> crate::core::PyStructureHandle {
        self.owner.into()
    }

    /// Number of rigid ligand candidates retained by the batch.
    #[getter]
    fn pose_count(&self) -> usize {
        self.poses.len()
    }

    /// Analytic instances generated when the batch is visible.
    #[getter]
    fn instance_count(&self) -> usize {
        self.template
            .instances_per_pose()
            .saturating_mul(self.poses.len())
    }

    #[getter]
    fn visible(&self) -> bool {
        self.visible
    }

    /// Immutable topology shared by every candidate.
    #[getter]
    fn template(&self) -> PyLicoriceTemplate {
        PyLicoriceTemplate(self.template.clone())
    }

    /// The rigid candidate column, in slot order.
    #[getter]
    fn poses(&self) -> Vec<PyLigandPose> {
        self.poses.iter().copied().map(PyLigandPose).collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "LigandPoseBatch(poses={}, visible={})",
            self.poses.len(),
            self.visible
        )
    }
}

//! Transported scientific output shares native scene validation.
use crate::scene_binding::PyScene;
use pyo3::prelude::*;

#[pyclass(name = "MolecularPose", frozen)]
pub(super) struct PyMolecularPose(pub(super) molgfx::source::MolecularPose);

#[pymethods]
impl PyMolecularPose {
    /// Cartesian interpolation fraction of the captured resident interval, if any.
    #[getter]
    fn interpolation(&self) -> Option<f64> {
        self.0.interpolation()
    }
}

#[pymethods]
impl PyScene {
    /// Retains the exact scientific calculation input without copying its coordinate owners.
    #[pyo3(signature = (*, structure))]
    fn molecular_pose(&self, structure: &Bound<'_, PyAny>) -> PyResult<PyMolecularPose> {
        self.inner
            .molecular_pose(molgfx::StructureId::new(crate::id_binding::structure_id(
                structure,
            )?))
            .map(PyMolecularPose)
            .map_err(crate::binding::error)
    }

    /// Binds actual provider metadata and triangle data encoded as molecular-surface JSON.
    #[pyo3(signature = (identity, data, *, pose=None))]
    fn bind_molecular_surface(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        identity: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
        pose: Option<&PyMolecularPose>,
    ) -> PyResult<()> {
        let identity =
            molgfx::RepresentationId::new(crate::id_binding::representation_id(identity)?);
        let data = surface_data(data)?;
        crate::runtime_binding::commit(slf, py, |scene| {
            match pose {
                Some(pose) => scene.bind_molecular_surface_data_at_pose(identity, data, &pose.0),
                None => scene.bind_molecular_surface_data(identity, data),
            }
            .map_err(crate::binding::error)
        })
    }

    /// Molecular representation identities awaiting their actual scientific geometry.
    fn unresolved_representations(&self) -> Vec<crate::id_binding::PyRepresentationId> {
        self.inner
            .unresolved_representations()
            .into_iter()
            .map(|identity| crate::id_binding::PyRepresentationId(identity.get()))
            .collect()
    }
}

fn surface_data(data: &Bound<'_, PyAny>) -> PyResult<molgfx::MolecularSurfaceData> {
    if let Ok(json) = data.extract::<String>() {
        return serde_json::from_str(&json)
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()));
    }
    let method = match data.getattr("method")?.extract::<String>()?.as_str() {
        "surf" => molgfx::MolecularSurfaceMethod::Surf,
        "msms" => molgfx::MolecularSurfaceMethod::Msms,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "surface method must be surf or msms",
            ));
        }
    };
    Ok(molgfx::MolecularSurfaceData {
        method,
        controls: molgfx::MolecularSurfaceControls {
            probe_radius: data.getattr("probe_radius")?.extract()?,
            density: data.getattr("density")?.extract()?,
            all_components: data.getattr("all_components")?.extract()?,
        },
        mesh: molgfx::schema::MolecularSurfaceDescription {
            discarded_degenerate_faces: data.getattr("discarded_degenerate_faces")?.extract()?,
            vertices: triples(data, "vertices")?,
            normals: triples(data, "normals")?,
            faces: triples(data, "faces")?,
            atom_rows: {
                let value = data.getattr("atom_rows")?;
                let array = value.extract::<numpy::PyReadonlyArray1<'_, u32>>()?;
                array.as_array().iter().copied().collect()
            },
        },
    })
}

fn triples<T: numpy::Element + Copy>(data: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<[T; 3]>> {
    let value = data.getattr(name)?;
    let array = value.extract::<numpy::PyReadonlyArray2<'_, T>>()?;
    let view = array.as_array();
    if view.ncols() != 3 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "surface columns require three components",
        ));
    }
    Ok(view
        .rows()
        .into_iter()
        .map(|row| [row[0], row[1], row[2]])
        .collect())
}

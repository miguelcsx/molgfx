//! Fixed-width GPU instruction record adapter.

use pyo3::prelude::*;

#[pyclass(name = "VisualInstructionGpu", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualInstructionGpu(pub(crate) molgfx::VisualInstructionGpu);

#[pymethods]
impl PyVisualInstructionGpu {
    #[new]
    fn new(control: [u32; 4], data: [f32; 4]) -> Self {
        Self(molgfx::VisualInstructionGpu { control, data })
    }

    #[getter]
    fn control(&self) -> [u32; 4] {
        self.0.control
    }

    #[getter]
    fn data(&self) -> [f32; 4] {
        self.0.data
    }

    #[staticmethod]
    fn byte_size() -> usize {
        std::mem::size_of::<molgfx::VisualInstructionGpu>()
    }
}

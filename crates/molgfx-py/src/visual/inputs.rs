//! Literal and built-in input constructors for the Python visual builder.

use super::{PyBoolExpr, PyColorExpr, PyScalarExpr, PyVectorExpr, PyVisualProgramBuilder};
use crate::error::visual;
use pyo3::prelude::*;

#[pymethods]
impl PyVisualProgramBuilder {
    #[new]
    fn new() -> Self {
        Self {
            inner: Some(molgfx::core::VisualProgramBuilder::new()),
        }
    }
    fn scalar(&mut self, value_: f32) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.scalar(value_)).map(PyScalarExpr)
    }
    fn color(&mut self, value_: (f32, f32, f32, f32)) -> PyResult<PyColorExpr> {
        visual(
            self.builder()?
                .color([value_.0, value_.1, value_.2, value_.3]),
        )
        .map(PyColorExpr)
    }
    fn vector(&mut self, value_: (f32, f32, f32)) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.vector([value_.0, value_.1, value_.2])).map(PyVectorExpr)
    }
    fn boolean(&mut self, value_: bool) -> PyResult<PyBoolExpr> {
        visual(self.builder()?.boolean(value_)).map(PyBoolExpr)
    }
    fn base_color(&mut self) -> PyResult<PyColorExpr> {
        visual(self.builder()?.base_color()).map(PyColorExpr)
    }
    fn base_opacity(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.base_opacity()).map(PyScalarExpr)
    }
    fn time(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.time()).map(PyScalarExpr)
    }
    fn local_position(&mut self) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.local_position()).map(PyVectorExpr)
    }
    fn world_position(&mut self) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.world_position()).map(PyVectorExpr)
    }
    fn normal(&mut self) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.normal()).map(PyVectorExpr)
    }
    fn view_direction(&mut self) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.view_direction()).map(PyVectorExpr)
    }
    fn camera_distance(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.camera_distance()).map(PyScalarExpr)
    }
    fn entity_index(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.entity_index()).map(PyScalarExpr)
    }
}

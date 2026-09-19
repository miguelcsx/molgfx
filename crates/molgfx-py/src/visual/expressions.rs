//! Typed visual expression and parameter handles exposed to Python.

use pyo3::prelude::*;

macro_rules! expression {
    ($python:literal, $name:ident, $native:ident) => {
        #[pyclass(name = $python, frozen, from_py_object)]
        #[derive(Clone, Copy, Debug)]
        pub(crate) struct $name(pub(crate) molgfx::core::$native);
    };
}

expression!("ScalarExpr", PyScalarExpr, ScalarExpr);
expression!("ColorExpr", PyColorExpr, ColorExpr);
expression!("BoolExpr", PyBoolExpr, BoolExpr);
expression!("VectorExpr", PyVectorExpr, VectorExpr);
expression!("ScalarParameter", PyScalarParameter, ScalarParameter);
expression!("ColorParameter", PyColorParameter, ColorParameter);
expression!("VectorParameter", PyVectorParameter, VectorParameter);

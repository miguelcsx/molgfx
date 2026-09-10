//! Registration for the visual-program Python surface.

use super::{
    Bound, PyBoolExpr, PyColorExpr, PyColorParameter, PyModule, PyModuleMethods, PyResult,
    PyScalarExpr, PyScalarParameter, PyVectorExpr, PyVectorParameter, PyVisualCompatibility,
    PyVisualEvaluation, PyVisualInputs, PyVisualOutput, PyVisualProgram, PyVisualProgramBuilder,
    PyVisualStage, PyVisualStyle,
};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("MAX_CLIP_PLANES", pdviewx::MAX_CLIP_PLANES)?;
    module.add(
        "MAX_VOLUME_TRANSFER_POINTS",
        pdviewx::MAX_VOLUME_TRANSFER_POINTS,
    )?;
    module.add("MAX_VISUAL_INSTRUCTIONS", pdviewx::MAX_VISUAL_INSTRUCTIONS)?;
    module.add("MAX_VISUAL_PROPERTIES", pdviewx::MAX_VISUAL_PROPERTIES)?;
    module.add("MAX_VISUAL_PARAMETERS", pdviewx::MAX_VISUAL_PARAMETERS)?;
    module.add_class::<PyVisualOutput>()?;
    module.add_class::<PyVisualStage>()?;
    module.add_class::<PyVisualCompatibility>()?;
    module.add_class::<PyVisualInputs>()?;
    module.add_class::<PyVisualEvaluation>()?;
    module.add_class::<PyScalarExpr>()?;
    module.add_class::<PyColorExpr>()?;
    module.add_class::<PyBoolExpr>()?;
    module.add_class::<PyVectorExpr>()?;
    module.add_class::<PyScalarParameter>()?;
    module.add_class::<PyColorParameter>()?;
    module.add_class::<PyVectorParameter>()?;
    module.add_class::<PyVisualProgram>()?;
    module.add_class::<PyVisualStyle>()?;
    module.add_class::<PyVisualProgramBuilder>()
}

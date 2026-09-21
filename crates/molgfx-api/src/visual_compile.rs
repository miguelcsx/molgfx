//! Deterministic lowering for immutable visual expression DAGs.

use super::{BoolExpr, ColorExpr, ParameterValue, ScalarExpr, VisualStyle};
use crate::{Color, Error};
use std::collections::{BTreeMap, BTreeSet};

/// Earliest shader stage required by a visual program.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisualStage {
    /// Constants and dynamic uniform parameters only.
    Uniform,
    /// Molecular properties or interaction-state inputs.
    Entity,
    /// Fragment-only evaluation.
    Fragment,
}

/// Validated, specialized visual program ready for a shader cache.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompiledVisual {
    hash: String,
    stage: VisualStage,
    properties: Vec<Box<str>>,
    parameters: Vec<Box<str>>,
    wgsl: String,
}

impl CompiledVisual {
    /// Stable source-program hash.
    #[must_use]
    pub fn stable_hash(&self) -> &str {
        &self.hash
    }

    /// Earliest required evaluation stage.
    #[must_use]
    pub const fn stage(&self) -> VisualStage {
        self.stage
    }

    /// Sorted molecular-property inputs.
    #[must_use]
    pub fn properties(&self) -> &[Box<str>] {
        &self.properties
    }

    /// Sorted dynamic uniform inputs.
    #[must_use]
    pub fn parameters(&self) -> &[Box<str>] {
        &self.parameters
    }

    /// Specialized WGSL functions for the three style outputs.
    #[must_use]
    pub fn wgsl(&self) -> &str {
        &self.wgsl
    }

    /// Deterministic compiler-plan summary.
    #[must_use]
    pub fn explain(&self) -> String {
        format!(
            "VisualStyle\nhash: {}\nstage: {:?}\nproperties: {}\nparameters: {}\ninterpreter: false",
            self.hash,
            self.stage,
            self.properties.join(", "),
            self.parameters.join(", ")
        )
    }
}

impl VisualStyle {
    /// Validates, folds, analyzes and lowers this expression DAG.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for non-finite constants,
    /// malformed domains, or names that cannot safely become WGSL bindings.
    pub fn compile(&self) -> Result<CompiledVisual, Error> {
        let mut inputs = Inputs::default();
        let color = color_wgsl(&self.color, &mut inputs)?;
        let opacity = scalar_wgsl(&self.opacity.clone().canonical(), &mut inputs)?;
        let visible = bool_wgsl(&self.visible.clone().canonical(), &mut inputs)?;
        let stage = if inputs.fragment {
            VisualStage::Fragment
        } else if inputs.entity {
            VisualStage::Entity
        } else {
            VisualStage::Uniform
        };
        let hash = self.stable_hash();
        let wgsl = format!(
            "// visual-style {hash}\nfn visual_color() -> vec4<f32> {{ return {color}; }}\nfn visual_opacity() -> f32 {{ return {opacity}; }}\nfn visual_visible() -> bool {{ return {visible}; }}\n"
        );
        Ok(CompiledVisual {
            hash,
            stage,
            properties: inputs.properties.into_iter().collect(),
            parameters: inputs.parameters.into_keys().collect(),
            wgsl,
        })
    }
}

#[derive(Default)]
struct Inputs {
    properties: BTreeSet<Box<str>>,
    parameters: BTreeMap<Box<str>, ParameterValue>,
    entity: bool,
    fragment: bool,
}

fn scalar_wgsl(expression: &ScalarExpr, inputs: &mut Inputs) -> Result<String, Error> {
    Ok(match expression {
        ScalarExpr::Constant(value) => finite(*value)?,
        ScalarExpr::Property(name) => {
            identifier(name)?;
            inputs.entity = true;
            let _ = inputs.properties.insert(name.clone());
            format!("properties.{name}")
        }
        ScalarExpr::Parameter(parameter) => {
            identifier(parameter.name())?;
            validate_scalar(*parameter.default_value())?;
            inputs.parameter(
                parameter.name(),
                ParameterValue::Scalar(*parameter.default_value()),
            )?;
            format!("parameters.{}", parameter.name())
        }
        ScalarExpr::Add(left, right) => format!(
            "({} + {})",
            scalar_wgsl(left, inputs)?,
            scalar_wgsl(right, inputs)?
        ),
        ScalarExpr::Multiply(left, right) => format!(
            "({} * {})",
            scalar_wgsl(left, inputs)?,
            scalar_wgsl(right, inputs)?
        ),
        ScalarExpr::Clamp {
            value,
            minimum,
            maximum,
        } => {
            validate_domain(*minimum, *maximum)?;
            format!(
                "clamp({}, {}, {})",
                scalar_wgsl(value, inputs)?,
                finite(*minimum)?,
                finite(*maximum)?
            )
        }
        ScalarExpr::VectorDot(left, right) => format!(
            "dot({}, {})",
            vector_wgsl(left, inputs)?,
            vector_wgsl(right, inputs)?
        ),
    })
}

fn vector_wgsl(expression: &super::VectorExpr, inputs: &mut Inputs) -> Result<String, Error> {
    use super::VectorExpr;
    Ok(match expression {
        VectorExpr::Constant(value) => format!(
            "vec3<f32>({}, {}, {})",
            finite(value[0])?,
            finite(value[1])?,
            finite(value[2])?
        ),
        VectorExpr::Property(name) => {
            identifier(name)?;
            inputs.entity = true;
            let _ = inputs.properties.insert(name.clone());
            format!("properties.{name}")
        }
        VectorExpr::Parameter(parameter) => {
            identifier(parameter.name())?;
            for value in parameter.default_value() {
                validate_scalar(*value)?;
            }
            inputs.parameter(
                parameter.name(),
                ParameterValue::Vector(*parameter.default_value()),
            )?;
            format!("parameters.{}", parameter.name())
        }
        VectorExpr::Add(left, right) => format!(
            "({} + {})",
            vector_wgsl(left, inputs)?,
            vector_wgsl(right, inputs)?
        ),
        VectorExpr::Scale(value, scale) => format!(
            "({} * {})",
            vector_wgsl(value, inputs)?,
            scalar_wgsl(scale, inputs)?
        ),
        VectorExpr::Normalize(value) => {
            format!("safe_normalize({})", vector_wgsl(value, inputs)?)
        }
    })
}

fn bool_wgsl(expression: &BoolExpr, inputs: &mut Inputs) -> Result<String, Error> {
    Ok(match expression {
        BoolExpr::Constant(value) => value.to_string(),
        BoolExpr::State(name) => {
            identifier(name)?;
            inputs.entity = true;
            format!("interaction.{name}")
        }
        BoolExpr::Less(left, right) => format!(
            "({} < {})",
            scalar_wgsl(left, inputs)?,
            scalar_wgsl(right, inputs)?
        ),
        BoolExpr::And(left, right) => format!(
            "({} && {})",
            bool_wgsl(left, inputs)?,
            bool_wgsl(right, inputs)?
        ),
        BoolExpr::Or(left, right) => format!(
            "({} || {})",
            bool_wgsl(left, inputs)?,
            bool_wgsl(right, inputs)?
        ),
        BoolExpr::Not(value) => format!("!({})", bool_wgsl(value, inputs)?),
    })
}

fn color_wgsl(expression: &ColorExpr, inputs: &mut Inputs) -> Result<String, Error> {
    Ok(match expression {
        ColorExpr::Constant(color) => color_literal(*color),
        ColorExpr::Parameter(parameter) => {
            identifier(parameter.name())?;
            inputs.parameter(
                parameter.name(),
                ParameterValue::Color(*parameter.default_value()),
            )?;
            format!("parameters.{}", parameter.name())
        }
        ColorExpr::Select { condition, yes, no } => format!(
            "select({}, {}, {})",
            color_wgsl(no, inputs)?,
            color_wgsl(yes, inputs)?,
            bool_wgsl(condition, inputs)?
        ),
        ColorExpr::Ramp {
            value,
            palette,
            domain,
            missing,
        } => {
            identifier(palette)?;
            validate_domain(domain[0], domain[1])?;
            inputs.fragment = true;
            format!(
                "ramp_{palette}({}, vec2<f32>({}, {}), {})",
                scalar_wgsl(value, inputs)?,
                finite(domain[0])?,
                finite(domain[1])?,
                color_literal(*missing)
            )
        }
    })
}

impl Inputs {
    fn parameter(&mut self, name: &str, value: ParameterValue) -> Result<(), Error> {
        match self.parameters.get(name) {
            Some(existing) if existing != &value => Err(Error::InvalidSpec(format!(
                "visual parameter '{name}' has conflicting declarations"
            ))),
            Some(_) => Ok(()),
            None => {
                let _ = self.parameters.insert(name.into(), value);
                Ok(())
            }
        }
    }
}

fn color_literal(color: Color) -> String {
    let [red, green, blue, alpha] = color.to_linear_f32();
    format!("vec4<f32>({red:.8}, {green:.8}, {blue:.8}, {alpha:.8})")
}

fn validate_scalar(value: f32) -> Result<(), Error> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidSpec(
            "visual values must be finite".to_owned(),
        ))
    }
}

fn finite(value: f32) -> Result<String, Error> {
    validate_scalar(value)?;
    Ok(format!("{value:.8}"))
}

fn validate_domain(minimum: f32, maximum: f32) -> Result<(), Error> {
    if minimum.is_finite() && maximum.is_finite() && minimum < maximum {
        Ok(())
    } else {
        Err(Error::InvalidSpec(
            "visual domains must be finite and increasing".to_owned(),
        ))
    }
}

fn identifier(name: &str) -> Result<(), Error> {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return Err(Error::InvalidSpec(
            "visual input names cannot be empty".to_owned(),
        ));
    };
    if !(first == '_' || first.is_ascii_alphabetic())
        || characters.any(|character| !(character == '_' || character.is_ascii_alphanumeric()))
    {
        return Err(Error::InvalidSpec(format!(
            "visual input name {name:?} is not a WGSL identifier"
        )));
    }
    Ok(())
}

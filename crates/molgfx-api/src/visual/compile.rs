//! Deterministic lowering for immutable visual expression DAGs.

use super::{BoolExpr, ColorExpr, ParameterValue, ScalarExpr, VisualStyle};
use crate::property::registry::{ScalarInput, VectorInput};
use crate::spec::lowering::Lowering;
use crate::{Color, Error};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) use super::native::lower as lower_native;

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
#[derive(Clone, PartialEq, Debug)]
pub struct CompiledVisual {
    hash: String,
    stage: VisualStage,
    properties: Vec<Box<str>>,
    channels: Vec<Box<str>>,
    parameters: Vec<Box<str>>,
    parameter_defaults: Vec<(Box<str>, ParameterValue)>,
    wgsl: String,
}

pub(crate) struct PreparedVisual {
    native: molgfx_core::VisualStyle,
    resolved: ResolvedVisual,
}

impl PreparedVisual {
    pub(crate) fn into_parts(self) -> (molgfx_core::VisualStyle, ResolvedVisual) {
        (self.native, self.resolved)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedVisual {
    source_key: [super::intern::Key; 3],
    parameters: BTreeMap<Box<str>, ResolvedParameter>,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedParameter {
    pub(crate) slot: usize,
    pub(crate) default: ParameterValue,
}

impl ResolvedVisual {
    pub(crate) fn parameter(&self, name: &str) -> Option<&ResolvedParameter> {
        self.parameters.get(name)
    }

    pub(crate) fn matches(&self, style: &VisualStyle) -> bool {
        self.source_key == style.structural_key()
    }
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

    /// Sorted interaction channels this program reads.
    #[must_use]
    pub fn channels(&self) -> &[Box<str>] {
        &self.channels
    }

    /// Sorted dynamic uniform inputs.
    #[must_use]
    pub fn parameters(&self) -> &[Box<str>] {
        &self.parameters
    }

    pub(crate) fn parameter_defaults(&self) -> &[(Box<str>, ParameterValue)] {
        &self.parameter_defaults
    }

    /// Specialized WGSL functions for the three style outputs.
    #[must_use]
    pub fn wgsl(&self) -> &str {
        &self.wgsl
    }

    /// Deterministic compiler-plan summary.
    ///
    /// Reports what this compiler produced, not what the renderer will run with:
    /// the compiled form is the interpreter's input, and the engine selects a
    /// specialized pipeline over it where one is eligible and compiles.
    /// `Renderer::explain` carries the executed path.
    #[must_use]
    pub fn explain(&self) -> String {
        format!(
            "VisualStyle\nhash: {}\nstage: {:?}\nproperties: {}\nparameters: {}\ncompiled: typed-bytecode-interpreter-input\nspecialized-wgsl: emitted",
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
        let parameter_defaults: Vec<_> = inputs.parameters.into_iter().collect();
        let parameters = parameter_defaults
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        Ok(CompiledVisual {
            hash,
            stage,
            properties: inputs.properties.into_iter().collect(),
            channels: inputs.channels.into_iter().collect(),
            parameters,
            parameter_defaults,
            wgsl,
        })
    }

    pub(crate) fn prepare(
        &self,
        overrides: &BTreeMap<Box<str>, ParameterValue>,
        lowering: Lowering<'_>,
        structure: crate::StructureId,
    ) -> Result<PreparedVisual, Error> {
        let compiled = self.compile()?;
        let native = lower_native(self, &compiled, overrides, lowering, structure)?;
        let parameters = compiled
            .parameter_defaults()
            .iter()
            .enumerate()
            .map(|(slot, (name, default))| {
                (
                    name.clone(),
                    ResolvedParameter {
                        slot,
                        default: default.clone(),
                    },
                )
            })
            .collect();
        Ok(PreparedVisual {
            native,
            resolved: ResolvedVisual {
                source_key: self.structural_key(),
                parameters,
            },
        })
    }
}

#[derive(Default)]
struct Inputs {
    depth: usize,
    properties: BTreeSet<Box<str>>,
    channels: BTreeSet<Box<str>>,
    parameters: BTreeMap<Box<str>, ParameterValue>,
    entity: bool,
    fragment: bool,
}

fn scalar_wgsl(expression: &ScalarExpr, inputs: &mut Inputs) -> Result<String, Error> {
    inputs.enter()?;
    let emitted = match expression {
        ScalarExpr::Constant(value) => finite(*value)?,
        ScalarExpr::Property(property) => {
            identifier(property.name())?;
            inputs.entity = true;
            let _ = inputs.properties.insert(property.name().into());
            format!("properties.{}", property.name())
        }
        ScalarExpr::Input(name) => {
            let input = ScalarInput::parse(name)?;
            inputs.entity = true;
            format!("inputs.{}", input.name())
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
    };
    inputs.leave();
    Ok(emitted)
}

fn vector_wgsl(expression: &super::VectorExpr, inputs: &mut Inputs) -> Result<String, Error> {
    use super::VectorExpr;
    inputs.enter()?;
    let emitted = match expression {
        VectorExpr::Constant(value) => format!(
            "vec3<f32>({}, {}, {})",
            finite(value[0])?,
            finite(value[1])?,
            finite(value[2])?
        ),
        VectorExpr::Property(name) => {
            let input = VectorInput::parse(name)?;
            inputs.entity = true;
            format!("inputs.{}", input.name())
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
    };
    inputs.leave();
    Ok(emitted)
}

fn bool_wgsl(expression: &BoolExpr, inputs: &mut Inputs) -> Result<String, Error> {
    inputs.enter()?;
    let emitted = match expression {
        BoolExpr::Constant(value) => value.to_string(),
        BoolExpr::State(name) => {
            identifier(name)?;
            inputs.entity = true;
            let _ = inputs.channels.insert(name.clone());
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
    };
    inputs.leave();
    Ok(emitted)
}

fn color_wgsl(expression: &ColorExpr, inputs: &mut Inputs) -> Result<String, Error> {
    inputs.enter()?;
    let emitted = match expression {
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
            // The same registry the renderer lowering uses, so a style that
            // compiles here cannot fail on an unknown ramp further down.
            let _ = crate::color::palette(palette)?;
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
    };
    inputs.leave();
    Ok(emitted)
}

impl Inputs {
    /// Counts one level of nesting, refusing a graph too deep to walk.
    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        let _ = super::intern::check_depth(self.depth)?;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

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

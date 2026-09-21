//! Lowering from the public typed DAG to the renderer's bounded typed IR.

use crate::visual::{BoolExpr, ColorExpr, ParameterValue, ScalarExpr, VectorExpr, VisualStyle};
use crate::{Error, color};
use molgfx_core::{
    BoolExpr as NativeBool, ColorExpr as NativeColor, ScalarExpr as NativeScalar,
    VectorExpr as NativeVector, VisualProgramBuilder,
};
use std::collections::BTreeMap;

pub(super) fn lower(
    style: &VisualStyle,
    overrides: &BTreeMap<Box<str>, ParameterValue>,
) -> Result<molgfx_core::VisualStyle, Error> {
    let mut compiler = Compiler::new(overrides);
    let color = compiler.color(&style.color)?;
    let opacity = compiler.scalar(&style.opacity)?;
    let visible = compiler.boolean(&style.visible)?;
    compiler.builder.set_base_color(color).map_err(core_error)?;
    compiler.builder.set_opacity(opacity).map_err(core_error)?;
    compiler
        .builder
        .set_visibility(visible)
        .map_err(core_error)?;
    let program = compiler.builder.finish().map_err(core_error)?;
    if compiler.used_parameters.len() != overrides.len() {
        return Err(Error::InvalidSpec(
            "visual parameter override names an undeclared parameter".to_owned(),
        ));
    }
    let mut lowered = molgfx_core::VisualStyle::new(program);
    for (slot, value) in compiler.updates {
        match value {
            ParameterValue::Scalar(value) => {
                let parameter = lowered
                    .program()
                    .scalar_parameter(slot)
                    .ok_or_else(|| invalid_parameter_type(slot))?;
                lowered.set_scalar(parameter, value).map_err(core_error)?;
            }
            ParameterValue::Color(value) => {
                let parameter = lowered
                    .program()
                    .color_parameter(slot)
                    .ok_or_else(|| invalid_parameter_type(slot))?;
                lowered
                    .set_color(parameter, value.to_linear_f32())
                    .map_err(core_error)?;
            }
            ParameterValue::Vector(value) => {
                let parameter = lowered
                    .program()
                    .vector_parameter(slot)
                    .ok_or_else(|| invalid_parameter_type(slot))?;
                lowered.set_vector(parameter, value).map_err(core_error)?;
            }
        }
    }
    Ok(lowered)
}

struct Compiler<'a> {
    builder: VisualProgramBuilder,
    overrides: &'a BTreeMap<Box<str>, ParameterValue>,
    used_parameters: std::collections::BTreeSet<Box<str>>,
    updates: Vec<(usize, ParameterValue)>,
    declarations: BTreeMap<Box<str>, ParameterValue>,
    scalars: BTreeMap<String, NativeScalar>,
    vectors: BTreeMap<String, NativeVector>,
    colors: BTreeMap<String, NativeColor>,
    booleans: BTreeMap<String, NativeBool>,
    scalar_parameters: BTreeMap<Box<str>, NativeScalar>,
    vector_parameters: BTreeMap<Box<str>, NativeVector>,
    color_parameters: BTreeMap<Box<str>, NativeColor>,
}

impl<'a> Compiler<'a> {
    fn new(overrides: &'a BTreeMap<Box<str>, ParameterValue>) -> Self {
        Self {
            builder: VisualProgramBuilder::new(),
            overrides,
            used_parameters: std::collections::BTreeSet::new(),
            updates: Vec::new(),
            declarations: BTreeMap::new(),
            scalars: BTreeMap::new(),
            vectors: BTreeMap::new(),
            colors: BTreeMap::new(),
            booleans: BTreeMap::new(),
            scalar_parameters: BTreeMap::new(),
            vector_parameters: BTreeMap::new(),
            color_parameters: BTreeMap::new(),
        }
    }

    fn scalar(&mut self, expression: &ScalarExpr) -> Result<NativeScalar, Error> {
        let key = expression_key(expression)?;
        if let Some(value) = self.scalars.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            ScalarExpr::Constant(value) => self.builder.scalar(*value),
            ScalarExpr::Property(name) => {
                let value = self.scalar_input(name)?;
                let _ = self.scalars.insert(key, value);
                return Ok(value);
            }
            ScalarExpr::Parameter(parameter) => {
                self.declare(
                    parameter.name(),
                    ParameterValue::Scalar(*parameter.default_value()),
                )?;
                if let Some(value) = self.scalar_parameters.get(parameter.name()).copied() {
                    Ok(value)
                } else {
                    if self.vector_parameters.contains_key(parameter.name())
                        || self.color_parameters.contains_key(parameter.name())
                    {
                        return Err(parameter_type_conflict(parameter.name()));
                    }
                    let slot = self.parameter_count();
                    let (_, value) = self
                        .builder
                        .scalar_parameter(*parameter.default_value())
                        .map_err(core_error)?;
                    let _ = self
                        .scalar_parameters
                        .insert(parameter.name().into(), value);
                    self.record_override(parameter.name(), slot, ParameterKind::Scalar)?;
                    Ok(value)
                }
            }
            ScalarExpr::Add(left, right) => {
                let left = self.scalar(left)?;
                let right = self.scalar(right)?;
                self.builder.add(left, right)
            }
            ScalarExpr::Multiply(left, right) => {
                let left = self.scalar(left)?;
                let right = self.scalar(right)?;
                self.builder.multiply(left, right)
            }
            ScalarExpr::Clamp {
                value,
                minimum,
                maximum,
            } => {
                let value = self.scalar(value)?;
                let minimum = self.builder.scalar(*minimum).map_err(core_error)?;
                let maximum = self.builder.scalar(*maximum).map_err(core_error)?;
                self.builder.clamp(value, minimum, maximum)
            }
            ScalarExpr::VectorDot(left, right) => {
                let left = self.vector(left)?;
                let right = self.vector(right)?;
                self.builder.dot(left, right)
            }
        }
        .map_err(core_error)?;
        let _ = self.scalars.insert(key, value);
        Ok(value)
    }

    fn scalar_input(&mut self, name: &str) -> Result<NativeScalar, Error> {
        let value = match name {
            "time" => self.builder.time(),
            "camera_distance" => self.builder.camera_distance(),
            "entity_index" => self.builder.entity_index(),
            "base_opacity" => self.builder.base_opacity(),
            "roughness" => self.builder.base_roughness(),
            "specular" => self.builder.base_specular(),
            "material_strength" => self.builder.base_material_strength(),
            _ => return Err(unsupported("the requested scalar property")),
        };
        value.map_err(core_error)
    }

    fn vector(&mut self, expression: &VectorExpr) -> Result<NativeVector, Error> {
        let key = expression_key(expression)?;
        if let Some(value) = self.vectors.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            VectorExpr::Constant(value) => self.builder.vector(*value),
            VectorExpr::Property(name) => match name.as_ref() {
                "local_position" => self.builder.local_position(),
                "world_position" => self.builder.world_position(),
                "normal" => self.builder.normal(),
                "view_direction" => self.builder.view_direction(),
                _ => return Err(unsupported("the requested vector property")),
            },
            VectorExpr::Parameter(parameter) => {
                self.declare(
                    parameter.name(),
                    ParameterValue::Vector(*parameter.default_value()),
                )?;
                if let Some(value) = self.vector_parameters.get(parameter.name()).copied() {
                    Ok(value)
                } else {
                    if self.scalar_parameters.contains_key(parameter.name())
                        || self.color_parameters.contains_key(parameter.name())
                    {
                        return Err(parameter_type_conflict(parameter.name()));
                    }
                    let slot = self.parameter_count();
                    let (_, value) = self
                        .builder
                        .vector_parameter(*parameter.default_value())
                        .map_err(core_error)?;
                    let _ = self
                        .vector_parameters
                        .insert(parameter.name().into(), value);
                    self.record_override(parameter.name(), slot, ParameterKind::Vector)?;
                    Ok(value)
                }
            }
            VectorExpr::Add(left, right) => {
                let left = self.vector(left)?;
                let right = self.vector(right)?;
                self.builder.add_vector(left, right)
            }
            VectorExpr::Scale(value, scale) => {
                let value = self.vector(value)?;
                let scale = self.scalar(scale)?;
                self.builder.scale_vector(value, scale)
            }
            VectorExpr::Normalize(value) => {
                let value = self.vector(value)?;
                self.builder.normalize(value)
            }
        }
        .map_err(core_error)?;
        let _ = self.vectors.insert(key, value);
        Ok(value)
    }

    fn boolean(&mut self, expression: &BoolExpr) -> Result<NativeBool, Error> {
        let key = expression_key(expression)?;
        if let Some(value) = self.booleans.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            BoolExpr::Constant(value) => self.builder.boolean(*value),
            BoolExpr::State(_) => return Err(unsupported("interaction-state visual input")),
            BoolExpr::Less(left, right) => {
                let left = self.scalar(left)?;
                let right = self.scalar(right)?;
                self.builder.less(left, right)
            }
            BoolExpr::And(left, right) => {
                let left = self.boolean(left)?;
                let right = self.boolean(right)?;
                self.builder.and(left, right)
            }
            BoolExpr::Or(left, right) => {
                let left = self.boolean(left)?;
                let right = self.boolean(right)?;
                self.builder.or(left, right)
            }
            BoolExpr::Not(value) => {
                let value = self.boolean(value)?;
                self.builder.not(value)
            }
        }
        .map_err(core_error)?;
        let _ = self.booleans.insert(key, value);
        Ok(value)
    }

    fn color(&mut self, expression: &ColorExpr) -> Result<NativeColor, Error> {
        let key = expression_key(expression)?;
        if let Some(value) = self.colors.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            ColorExpr::Constant(value) => self.builder.color(value.to_linear_f32()),
            ColorExpr::Parameter(parameter) => {
                self.declare(
                    parameter.name(),
                    ParameterValue::Color(*parameter.default_value()),
                )?;
                if let Some(value) = self.color_parameters.get(parameter.name()).copied() {
                    Ok(value)
                } else {
                    if self.scalar_parameters.contains_key(parameter.name())
                        || self.vector_parameters.contains_key(parameter.name())
                    {
                        return Err(parameter_type_conflict(parameter.name()));
                    }
                    let slot = self.parameter_count();
                    let (_, value) = self
                        .builder
                        .color_parameter(parameter.default_value().to_linear_f32())
                        .map_err(core_error)?;
                    let _ = self.color_parameters.insert(parameter.name().into(), value);
                    self.record_override(parameter.name(), slot, ParameterKind::Color)?;
                    Ok(value)
                }
            }
            ColorExpr::Select { condition, yes, no } => {
                let condition = self.boolean(condition)?;
                let yes = self.color(yes)?;
                let no = self.color(no)?;
                self.builder.select_color(condition, yes, no)
            }
            ColorExpr::Ramp {
                value,
                palette,
                domain,
                ..
            } => {
                let value = self.scalar(value)?;
                let colors = color::palette(palette).map(crate::Color::native);
                let ramp = molgfx_core::ScalarRamp::new(
                    [domain[0], domain[0].midpoint(domain[1]), domain[1]],
                    colors,
                )?;
                self.builder.ramp(value, ramp)
            }
        }
        .map_err(core_error)?;
        let _ = self.colors.insert(key, value);
        Ok(value)
    }

    fn parameter_count(&self) -> usize {
        self.scalar_parameters.len() + self.vector_parameters.len() + self.color_parameters.len()
    }

    fn declare(&mut self, name: &str, value: ParameterValue) -> Result<(), Error> {
        match self.declarations.get(name) {
            Some(existing) if existing != &value => Err(Error::InvalidSpec(format!(
                "visual parameter '{name}' has conflicting declarations"
            ))),
            Some(_) => Ok(()),
            None => {
                let _ = self.declarations.insert(name.into(), value);
                Ok(())
            }
        }
    }

    fn record_override(
        &mut self,
        name: &str,
        slot: usize,
        kind: ParameterKind,
    ) -> Result<(), Error> {
        let Some(value) = self.overrides.get(name) else {
            return Ok(());
        };
        let matches = matches!(
            (kind, value),
            (ParameterKind::Scalar, ParameterValue::Scalar(_))
                | (ParameterKind::Color, ParameterValue::Color(_))
                | (ParameterKind::Vector, ParameterValue::Vector(_))
        );
        if !matches {
            return Err(parameter_type_conflict(name));
        }
        let _ = self.used_parameters.insert(name.into());
        self.updates.push((slot, value.clone()));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ParameterKind {
    Scalar,
    Color,
    Vector,
}

fn expression_key(value: &impl serde::Serialize) -> Result<String, Error> {
    serde_json::to_string(value).map_err(Error::from)
}

fn unsupported(input: &str) -> Error {
    Error::InvalidSpec(format!("renderer does not expose {input} yet"))
}

fn parameter_type_conflict(name: &str) -> Error {
    Error::InvalidSpec(format!(
        "visual parameter '{name}' is used with incompatible types"
    ))
}

fn invalid_parameter_type(slot: usize) -> Error {
    Error::InvalidSpec(format!(
        "visual parameter slot {slot} has an incompatible type"
    ))
}

fn core_error(error: molgfx_core::VisualError) -> Error {
    molgfx_core::CoreError::from(error).into()
}

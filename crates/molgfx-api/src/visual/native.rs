//! Lowering from the public typed DAG to the renderer's bounded typed IR.

use super::intern::{Interner, Key};
use crate::property::registry::{ScalarInput, StateChannel, VectorInput};
use crate::spec::lowering::Lowering;
use crate::visual::{
    BoolExpr, ColorExpr, CompiledVisual, ParameterValue, ScalarExpr, VectorExpr, VisualStyle,
};
use crate::{Error, color};
use molgfx_core::{
    BoolExpr as NativeBool, ColorExpr as NativeColor, ScalarExpr as NativeScalar,
    VectorExpr as NativeVector, VisualProgramBuilder,
};
use std::collections::BTreeMap;
use std::collections::HashMap;

pub(crate) fn lower(
    style: &VisualStyle,
    compiled_plan: &CompiledVisual,
    overrides: &BTreeMap<Box<str>, ParameterValue>,
    lowering: Lowering<'_>,
    structure: crate::StructureId,
) -> Result<molgfx_core::VisualStyle, Error> {
    let mut lowerer = Compiler::new(compiled_plan, overrides, lowering, structure)?;
    let color = lowerer.color(&style.color)?;
    let opacity = lowerer.scalar(&style.opacity)?;
    let visible = lowerer.boolean(&style.visible)?;
    lowerer.builder.set_base_color(color).map_err(core_error)?;
    lowerer.builder.set_opacity(opacity).map_err(core_error)?;
    lowerer
        .builder
        .set_visibility(visible)
        .map_err(core_error)?;
    let program = lowerer.builder.finish().map_err(core_error)?;
    if lowerer.used_parameters.len() != overrides.len() {
        return Err(Error::InvalidSpec(
            "visual parameter override names an undeclared parameter".to_owned(),
        ));
    }
    let mut runtime_style = molgfx_core::VisualStyle::new(program);
    for (slot, value) in lowerer.updates {
        match value {
            ParameterValue::Scalar(value) => {
                let parameter = runtime_style
                    .program()
                    .scalar_parameter(slot)
                    .ok_or_else(|| invalid_parameter_type(slot))?;
                runtime_style
                    .set_scalar(parameter, value)
                    .map_err(core_error)?;
            }
            ParameterValue::Color(value) => {
                let parameter = runtime_style
                    .program()
                    .color_parameter(slot)
                    .ok_or_else(|| invalid_parameter_type(slot))?;
                runtime_style
                    .set_color(parameter, value.to_linear_f32())
                    .map_err(core_error)?;
            }
            ParameterValue::Vector(value) => {
                let parameter = runtime_style
                    .program()
                    .vector_parameter(slot)
                    .ok_or_else(|| invalid_parameter_type(slot))?;
                runtime_style
                    .set_vector(parameter, value)
                    .map_err(core_error)?;
            }
        }
    }
    Ok(runtime_style)
}

struct Compiler<'a> {
    builder: VisualProgramBuilder,
    overrides: &'a BTreeMap<Box<str>, ParameterValue>,
    used_parameters: std::collections::BTreeSet<Box<str>>,
    updates: Vec<(usize, ParameterValue)>,
    scalars: HashMap<Key, NativeScalar>,
    vectors: HashMap<Key, NativeVector>,
    colors: HashMap<Key, NativeColor>,
    booleans: HashMap<Key, NativeBool>,
    scalar_parameters: BTreeMap<Box<str>, NativeScalar>,
    vector_parameters: BTreeMap<Box<str>, NativeVector>,
    color_parameters: BTreeMap<Box<str>, NativeColor>,
    interner: Interner,
    properties: &'a BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    channels: &'a [Box<str>],
    structure: crate::StructureId,
}

impl<'a> Compiler<'a> {
    fn new(
        compiled_plan: &CompiledVisual,
        overrides: &'a BTreeMap<Box<str>, ParameterValue>,
        lowering: Lowering<'a>,
        structure: crate::StructureId,
    ) -> Result<Self, Error> {
        let mut lowerer = Self {
            builder: VisualProgramBuilder::new(),
            overrides,
            used_parameters: std::collections::BTreeSet::new(),
            updates: Vec::new(),
            scalars: HashMap::new(),
            vectors: HashMap::new(),
            colors: HashMap::new(),
            booleans: HashMap::new(),
            scalar_parameters: BTreeMap::new(),
            vector_parameters: BTreeMap::new(),
            color_parameters: BTreeMap::new(),
            interner: Interner::default(),
            properties: lowering.properties,
            channels: lowering.channels,
            structure,
        };
        for (name, default) in compiled_plan.parameter_defaults() {
            lowerer.declare_parameter(name, default)?;
        }
        Ok(lowerer)
    }

    fn declare_parameter(&mut self, name: &str, default: &ParameterValue) -> Result<(), Error> {
        let slot = self.parameter_count();
        match default {
            ParameterValue::Scalar(value) => {
                let (_, expression) = self.builder.scalar_parameter(*value).map_err(core_error)?;
                let _ = self.scalar_parameters.insert(name.into(), expression);
                self.record_override(name, slot, ParameterKind::Scalar)
            }
            ParameterValue::Color(value) => {
                let (_, expression) = self
                    .builder
                    .color_parameter(value.to_linear_f32())
                    .map_err(core_error)?;
                let _ = self.color_parameters.insert(name.into(), expression);
                self.record_override(name, slot, ParameterKind::Color)
            }
            ParameterValue::Vector(value) => {
                let (_, expression) = self.builder.vector_parameter(*value).map_err(core_error)?;
                let _ = self.vector_parameters.insert(name.into(), expression);
                self.record_override(name, slot, ParameterKind::Vector)
            }
        }
    }

    fn scalar(&mut self, expression: &ScalarExpr) -> Result<NativeScalar, Error> {
        let key = self.interner.scalar(expression);
        if let Some(value) = self.scalars.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            ScalarExpr::Constant(value) => self.builder.scalar(*value),
            ScalarExpr::Property(property) => {
                if property.structure() != self.structure {
                    return Err(Error::InvalidSpec(format!(
                        "property '{}' belongs to another structure",
                        property.name()
                    )));
                }
                let handle = self
                    .properties
                    .get(property.name())
                    .copied()
                    .ok_or_else(|| {
                        Error::InvalidSpec(format!("property '{}' is not bound", property.name()))
                    })?;
                let value = self.builder.atom_property(handle).map_err(core_error)?;
                let _ = self.scalars.insert(key, value);
                return Ok(value);
            }
            ScalarExpr::Input(name) => {
                let value = self.scalar_input(name)?;
                let _ = self.scalars.insert(key, value);
                return Ok(value);
            }
            ScalarExpr::Parameter(parameter) => {
                return self
                    .scalar_parameters
                    .get(parameter.name())
                    .copied()
                    .ok_or_else(|| parameter_type_conflict(parameter.name()));
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
        let value = match ScalarInput::parse(name)? {
            ScalarInput::Time => self.builder.time(),
            ScalarInput::CameraDistance => self.builder.camera_distance(),
            ScalarInput::EntityIndex => self.builder.entity_index(),
            ScalarInput::BaseOpacity => self.builder.base_opacity(),
            ScalarInput::Roughness => self.builder.base_roughness(),
            ScalarInput::Specular => self.builder.base_specular(),
            ScalarInput::MaterialStrength => self.builder.base_material_strength(),
        };
        value.map_err(core_error)
    }

    fn vector(&mut self, expression: &VectorExpr) -> Result<NativeVector, Error> {
        let key = self.interner.vector(expression);
        if let Some(value) = self.vectors.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            VectorExpr::Constant(value) => self.builder.vector(*value),
            VectorExpr::Property(name) => match VectorInput::parse(name)? {
                VectorInput::LocalPosition => self.builder.local_position(),
                VectorInput::WorldPosition => self.builder.world_position(),
                VectorInput::Normal => self.builder.normal(),
                VectorInput::ViewDirection => self.builder.view_direction(),
            },
            VectorExpr::Parameter(parameter) => {
                return self
                    .vector_parameters
                    .get(parameter.name())
                    .copied()
                    .ok_or_else(|| parameter_type_conflict(parameter.name()));
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
        let key = self.interner.boolean(expression);
        if let Some(value) = self.booleans.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            BoolExpr::Constant(value) => self.builder.boolean(*value),
            BoolExpr::State(name) => {
                let mask = StateChannel::parse(name, self.channels)?.mask()?;
                self.builder.interaction_state(mask)
            }
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
        let key = self.interner.color(expression);
        if let Some(value) = self.colors.get(&key).copied() {
            return Ok(value);
        }
        let value = match expression {
            ColorExpr::Constant(value) => self.builder.color(value.to_linear_f32()),
            ColorExpr::Parameter(parameter) => {
                return self
                    .color_parameters
                    .get(parameter.name())
                    .copied()
                    .ok_or_else(|| parameter_type_conflict(parameter.name()));
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
                let colors = color::palette(palette)?.map(crate::Color::native);
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

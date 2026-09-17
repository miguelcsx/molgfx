//! Caller-owned per-atom columns, the hook for arbitrary science.
//!
//! A program reads a column the caller uploaded rather than a quantity this
//! library knows how to compute, which is what lets a new metric drive the
//! picture without the engine learning about it.

use super::super::numeric::encode_slot;
use super::super::{
    MAX_VISUAL_PROPERTIES, Opcode, ScalarExpr, ValueKind, VisualError, VisualStage,
};
use super::VisualProgramBuilder;
use crate::{
    AtomPropertyHandle, AttributeHandle, AttributeKind, VisualAttributeRef, VisualColumnKey,
};

impl VisualProgramBuilder {
    /// Caller-owned atom property. Missing values remain NaN for explicit handling.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program already references its limit of
    /// property columns, or has no instruction slot left.
    pub fn atom_property(
        &mut self,
        property: AtomPropertyHandle,
    ) -> Result<ScalarExpr, VisualError> {
        let reference = VisualAttributeRef::LegacyScalar(property);
        let slot = if let Some(index) = self.attributes.iter().position(|value| *value == reference)
        {
            index
        } else {
            if self.attributes.len() == MAX_VISUAL_PROPERTIES {
                return Err(VisualError::PropertyLimit);
            }
            self.properties.push(property);
            self.attributes.push(reference);
            self.attributes.len() - 1
        };
        self.emit_at(
            ValueKind::Scalar,
            Opcode::Property,
            [0; 3],
            [encode_slot(slot), 0.0, 0.0, 0.0],
            VisualStage::Entity,
        )
        .map(ScalarExpr)
    }

    /// Caller-owned scalar attribute in the program's exact row domain.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no attribute or instruction
    /// capacity left.
    pub fn scalar_attribute(
        &mut self,
        attribute: AttributeHandle,
    ) -> Result<ScalarExpr, VisualError> {
        self.attribute(attribute, AttributeKind::Scalar, ValueKind::Scalar)
            .map(ScalarExpr)
    }

    /// Caller-owned category id exposed as an exact scalar value.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no attribute or instruction
    /// capacity left.
    pub fn category_attribute(
        &mut self,
        attribute: AttributeHandle,
    ) -> Result<ScalarExpr, VisualError> {
        self.attribute(attribute, AttributeKind::Category, ValueKind::Scalar)
            .map(ScalarExpr)
    }

    /// Caller-owned tightly packed vector attribute.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no attribute or instruction
    /// capacity left.
    pub fn vector_attribute(
        &mut self,
        attribute: AttributeHandle,
    ) -> Result<super::super::VectorExpr, VisualError> {
        self.attribute(attribute, AttributeKind::Vector, ValueKind::Vector)
            .map(super::super::VectorExpr)
    }

    /// Caller-owned packed RGBA8 attribute.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when the program has no attribute or instruction
    /// capacity left.
    pub fn color_attribute(
        &mut self,
        attribute: AttributeHandle,
    ) -> Result<super::super::ColorExpr, VisualError> {
        self.attribute(attribute, AttributeKind::Color, ValueKind::Color)
            .map(super::super::ColorExpr)
    }

    /// Scene-independent scalar column bound by a chunk visual descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when column or instruction capacity is exhausted.
    pub fn scalar_column(&mut self, key: VisualColumnKey) -> Result<ScalarExpr, VisualError> {
        self.column(key, AttributeKind::Scalar, ValueKind::Scalar)
            .map(ScalarExpr)
    }

    /// Scene-independent category column exposed as an exact scalar value.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when column or instruction capacity is exhausted.
    pub fn category_column(&mut self, key: VisualColumnKey) -> Result<ScalarExpr, VisualError> {
        self.column(key, AttributeKind::Category, ValueKind::Scalar)
            .map(ScalarExpr)
    }

    /// Scene-independent tightly packed vector column.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when column or instruction capacity is exhausted.
    pub fn vector_column(
        &mut self,
        key: VisualColumnKey,
    ) -> Result<super::super::VectorExpr, VisualError> {
        self.column(key, AttributeKind::Vector, ValueKind::Vector)
            .map(super::super::VectorExpr)
    }

    /// Scene-independent packed RGBA8 column.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError`] when column or instruction capacity is exhausted.
    pub fn color_column(
        &mut self,
        key: VisualColumnKey,
    ) -> Result<super::super::ColorExpr, VisualError> {
        self.column(key, AttributeKind::Color, ValueKind::Color)
            .map(super::super::ColorExpr)
    }

    fn attribute(
        &mut self,
        handle: AttributeHandle,
        physical: AttributeKind,
        value: ValueKind,
    ) -> Result<super::super::Expr, VisualError> {
        self.emit_attribute(
            VisualAttributeRef::Attribute {
                handle,
                kind: physical,
            },
            physical,
            value,
        )
    }

    fn column(
        &mut self,
        key: VisualColumnKey,
        physical: AttributeKind,
        value: ValueKind,
    ) -> Result<super::super::Expr, VisualError> {
        self.emit_attribute(
            VisualAttributeRef::Column {
                key,
                kind: physical,
            },
            physical,
            value,
        )
    }

    fn emit_attribute(
        &mut self,
        reference: VisualAttributeRef,
        physical: AttributeKind,
        value: ValueKind,
    ) -> Result<super::super::Expr, VisualError> {
        let slot = if let Some(index) = self.attributes.iter().position(|item| *item == reference) {
            index
        } else {
            if self.attributes.len() == MAX_VISUAL_PROPERTIES {
                return Err(VisualError::PropertyLimit);
            }
            self.attributes.push(reference);
            self.attributes.len() - 1
        };
        self.emit_at(
            value,
            Opcode::Property,
            [0; 3],
            [encode_slot(slot), 1.0, encode_attribute_kind(physical), 0.0],
            VisualStage::Entity,
        )
    }
}

const fn encode_attribute_kind(kind: AttributeKind) -> f32 {
    match kind {
        AttributeKind::Scalar => 0.0,
        AttributeKind::Category => 1.0,
        AttributeKind::Vector => 2.0,
        AttributeKind::Color => 3.0,
    }
}

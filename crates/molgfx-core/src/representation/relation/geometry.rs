//! Instantaneous oriented geometry over borrowed atom-coordinate rows.

use super::{Relation, RelationBatch, SpatialAnchor, invalid, relation_dependencies};
use crate::{CoreError, RowDomain, RowEntityRef};
use std::sync::Arc;

/// Distance and angle gates for one source-backed connector.
///
/// Distances use source-local ångström coordinates, so a display transform does
/// not change scientific acceptance. Chemistry eligibility belongs to the caller.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct OrientedRelation {
    /// Vertex of the angle formed by the connector's start, pivot and end.
    pub pivot: RowEntityRef,
    /// Geometric acceptance over the current source coordinates.
    pub geometry: OrientedGeometry,
}

/// Validated geometric acceptance independent of molecular classifications.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct OrientedGeometry {
    /// Maximum start-to-end distance in ångström.
    pub maximum_endpoint_distance: f32,
    /// Minimum start–pivot–end angle in degrees.
    pub minimum_angle_degrees: f32,
    /// Optional maximum pivot-to-end distance in ångström.
    pub maximum_pivot_end_distance: Option<f32>,
}

impl OrientedRelation {
    fn validate(self, relation: Relation) -> Result<(), CoreError> {
        let (SpatialAnchor::Entity(start), SpatialAnchor::Entity(end)) =
            (relation.start, relation.end)
        else {
            return Err(invalid("oriented relations require atom endpoints"));
        };
        if !matches!(self.pivot.domain(), RowDomain::Atoms(_))
            || start.domain() != self.pivot.domain()
            || end.domain() != self.pivot.domain()
            || start.row() == self.pivot.row()
            || end.row() == self.pivot.row()
            || start.row() == end.row()
        {
            return Err(invalid(
                "oriented relations require three distinct atoms in one source",
            ));
        }
        self.geometry.validate()
    }
}

impl OrientedGeometry {
    /// Checks physical cutoffs before any coordinates or GPU resources are read.
    ///
    /// # Errors
    /// Distances must be positive with finite squared values; angles span zero
    /// through 180 degrees.
    pub fn validate(self) -> Result<(), CoreError> {
        if !valid_distance(self.maximum_endpoint_distance)
            || !self.minimum_angle_degrees.is_finite()
            || !(0.0..=180.0).contains(&self.minimum_angle_degrees)
            || self
                .maximum_pivot_end_distance
                .is_some_and(|value| !valid_distance(value))
        {
            return Err(invalid(
                "oriented relation geometry must be finite and bounded",
            ));
        }
        Ok(())
    }
}

impl RelationBatch {
    /// Installs one instantaneous distance/angle gate per logical relation row.
    ///
    /// # Errors
    /// Requires source-aligned geometry and three distinct atoms from the same
    /// structure. Cutoffs must be positive, with representable squared values.
    pub fn with_oriented_geometry(
        mut self,
        geometry: Arc<[OrientedRelation]>,
    ) -> Result<Self, CoreError> {
        if geometry.len() != self.relations.len() {
            return Err(invalid("oriented geometry must align with relation rows"));
        }
        for (&constraint, &relation) in geometry.iter().zip(self.relations.iter()) {
            constraint.validate(relation)?;
        }
        self.dependencies = relation_dependencies(&self.relations, Some(&geometry));
        self.geometry = Some(geometry);
        Ok(self)
    }

    /// Optional source-aligned instantaneous distance/angle gates.
    #[must_use]
    pub const fn oriented_geometry(&self) -> Option<&Arc<[OrientedRelation]>> {
        self.geometry.as_ref()
    }
}

fn valid_distance(value: f32) -> bool {
    value.is_finite() && value > 0.0 && (value * value).is_finite()
}

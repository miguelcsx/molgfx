//! Geometric measurements and their anchors.

use super::Anchor;
use serde::{Deserialize, Serialize};

/// Geometric measurement with exact arity encoded by its tagged form.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MeasurementSpec {
    /// Distance between two anchors.
    Distance {
        /// Ordered endpoints.
        anchors: [Anchor; 2],
    },
    /// Angle formed by three anchors.
    Angle {
        /// Ordered angle points.
        anchors: [Anchor; 3],
    },
    /// Signed torsion formed by four anchors.
    Dihedral {
        /// Ordered torsion points.
        anchors: [Anchor; 4],
    },
}

/// The anchors a measurement reads, in order.
#[must_use]
pub fn measurement_anchors(spec: &MeasurementSpec) -> &[Anchor] {
    match spec {
        MeasurementSpec::Distance { anchors } => anchors,
        MeasurementSpec::Angle { anchors } => anchors,
        MeasurementSpec::Dihedral { anchors } => anchors,
    }
}

/// The kind name and anchor count of a measurement.
#[must_use]
pub const fn measurement_shape(spec: &MeasurementSpec) -> (&'static str, usize) {
    match spec {
        MeasurementSpec::Distance { .. } => ("distance", 2),
        MeasurementSpec::Angle { .. } => ("angle", 3),
        MeasurementSpec::Dihedral { .. } => ("dihedral", 4),
    }
}

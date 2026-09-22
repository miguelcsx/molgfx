//! Curated constructors for scientific scene items.

/// Density-volume authoring.
pub mod density {
    use super::super::{Color, DataSource, Volume, VolumeSpec};

    /// Declares a density grid whose values will be supplied at runtime.
    #[must_use]
    pub fn volume(source: DataSource, dimensions: [u32; 3]) -> Volume {
        Volume(VolumeSpec {
            source,
            dimensions,
            spacing: [1.0; 3],
            origin: [0.0; 3],
            isovalue: 1.0,
            color: Color::rgb(49, 104, 142),
        })
    }
}

/// Label and annotation authoring.
pub mod annotation {
    use super::super::{Anchor, AnnotationSpec, Color, Label};

    /// Creates a text label anchored to world or molecular state.
    #[must_use]
    pub fn label(anchor: Anchor, text: impl Into<Box<str>>) -> Label {
        Label(AnnotationSpec {
            anchor,
            text: text.into(),
            color: Color::rgb(255, 255, 255),
        })
    }
}

/// Distance, angle, and dihedral authoring.
pub mod measurement {
    use super::super::{Anchor, MeasurementSpec};

    /// Measures the distance between two anchors.
    #[must_use]
    pub fn distance(a: Anchor, b: Anchor) -> MeasurementSpec {
        MeasurementSpec::Distance { anchors: [a, b] }
    }

    /// Measures the angle formed by three anchors.
    #[must_use]
    pub fn angle(a: Anchor, b: Anchor, c: Anchor) -> MeasurementSpec {
        MeasurementSpec::Angle { anchors: [a, b, c] }
    }

    /// Measures the signed dihedral formed by four anchors.
    #[must_use]
    pub fn dihedral(a: Anchor, b: Anchor, c: Anchor, d: Anchor) -> MeasurementSpec {
        MeasurementSpec::Dihedral {
            anchors: [a, b, c, d],
        }
    }
}

/// Caller-supplied scientific interactions.
pub mod interaction {
    use super::super::{Anchor, InteractionKind, ScientificInteractionSpec};

    /// Declares one known interaction between two semantic anchors.
    #[must_use]
    pub fn explicit(
        kind: InteractionKind,
        first: Anchor,
        second: Anchor,
    ) -> ScientificInteractionSpec {
        ScientificInteractionSpec::Explicit {
            kind,
            endpoints: [first, second],
        }
    }
}

/// Trajectory authoring.
pub mod trajectory {
    use super::super::{DataSource, StructureId, TrajectorySpec};

    /// Binds an external frame sequence to an existing structure topology.
    #[must_use]
    pub fn trajectory(
        structure: StructureId,
        source: DataSource,
        frame_count: u64,
    ) -> TrajectorySpec {
        TrajectorySpec {
            structure,
            source,
            frame_count,
            time_step: None,
            time_unit: None,
        }
    }
}

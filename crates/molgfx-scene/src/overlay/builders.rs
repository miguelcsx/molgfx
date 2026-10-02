//! Curated constructors for overlay scene items.

/// Density-volume authoring.
pub mod density {
    use super::super::{DataSource, Volume, VolumeSpec};
    use crate::Color;

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
    use super::super::{Anchor, AnnotationSpec, Label};
    use crate::Color;

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

/// Caller-supplied overlay interactions.
pub mod interaction {
    use super::super::{Anchor, InteractionKind, InteractionSpec};

    /// Declares one known interaction between two semantic anchors.
    #[must_use]
    pub fn explicit(kind: InteractionKind, first: Anchor, second: Anchor) -> InteractionSpec {
        InteractionSpec::Explicit {
            kind,
            endpoints: [first, second],
        }
    }
}

/// Trajectory authoring.
pub mod trajectory {
    use super::super::{DataSource, TrajectorySpec};
    use crate::StructureId;

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

/// Per-atom anisotropic-displacement ellipsoid authoring.
pub mod ellipsoid {
    use super::super::EllipsoidSpec;
    use crate::{Color, Selection, StructureId};

    /// Draws one ellipsoid per selected atom that carries a displacement tensor.
    ///
    /// The default scale is one standard deviation, the surface the recorded
    /// tensor already describes, and the default color is a neutral blue.
    #[must_use]
    pub fn adp(structure: StructureId, selection: Selection) -> EllipsoidSpec {
        EllipsoidSpec {
            structure,
            selection,
            scale: 1.0,
            color: Color::rgb(96, 132, 200),
            opacity: 1.0,
        }
    }
}

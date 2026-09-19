//! Caller-supplied molecular interactions and their deterministic glyph style.
//!
//! Detection, geometric classification and trajectory aggregation belong to
//! `molframe` or the caller. The render engine validates those facts and maps
//! them to a stable visual vocabulary; it never infers chemistry here.

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;

use crate::{CoreError, EntityRef, StructureHandle};
use molgfx_math::{Rgba8, Vec3};
use std::sync::Arc;

/// Chemically meaningful interaction class supplied by the caller.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionKind {
    /// Donor-to-acceptor hydrogen bond.
    HydrogenBond,
    /// Oppositely charged group contact.
    SaltBridge,
    /// Aromatic ring stacking interaction.
    PiStacking,
    /// Non-polar contact.
    Hydrophobic,
    /// Metal-to-ligand coordination.
    MetalCoordination,
}

/// Direction carried by an interaction, when its source establishes one.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InteractionDirection {
    /// The interaction has no directional meaning.
    #[default]
    Undirected,
    /// The glyph points from the first anchor to the second.
    Forward,
    /// The glyph points from the second anchor to the first.
    Reverse,
}

/// Repeating screen-space mark used for an interaction line.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionPattern {
    /// Continuous analytic line.
    Solid,
    /// Repeating line segments.
    Dashes,
    /// Repeating round marks.
    Dots,
    /// Continuous sinusoidal spring guide.
    Spring,
}

/// One already-resolved world-space interaction endpoint.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InteractionAnchor {
    position: Vec3,
    entity: Option<EntityRef>,
}

impl InteractionAnchor {
    /// Creates a free world-space anchor such as a ring or group centroid.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] for non-finite coordinates.
    pub fn world(position: Vec3) -> Result<Self, CoreError> {
        Self::new(position, None)
    }

    /// Creates an anchor associated with an atom or another scene entity.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] for non-finite coordinates.
    pub fn entity(position: Vec3, entity: EntityRef) -> Result<Self, CoreError> {
        Self::new(position, Some(entity))
    }

    fn new(position: Vec3, entity: Option<EntityRef>) -> Result<Self, CoreError> {
        if !position.is_finite() {
            return Err(CoreError::InvalidInteraction {
                reason: "anchor position must be finite",
            });
        }
        Ok(Self { position, entity })
    }

    /// Caller-resolved world-space position in Ångström.
    #[must_use]
    pub const fn position(self) -> Vec3 {
        self.position
    }

    /// Optional source entity represented by this anchor.
    #[must_use]
    pub const fn source_entity(self) -> Option<EntityRef> {
        self.entity
    }
}

/// Caller-computed geometry retained for inspection and labels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InteractionGeometry {
    distance_angstrom: f32,
    angle_degrees: Option<f32>,
}

impl InteractionGeometry {
    /// Stores a positive distance and optional angle in `[0, 180]`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] for malformed values.
    pub fn new(distance_angstrom: f32, angle_degrees: Option<f32>) -> Result<Self, CoreError> {
        if !distance_angstrom.is_finite() || distance_angstrom <= 0.0 {
            return Err(CoreError::InvalidInteraction {
                reason: "distance must be finite and positive",
            });
        }
        if angle_degrees.is_some_and(|angle| !angle.is_finite() || !(0.0..=180.0).contains(&angle))
        {
            return Err(CoreError::InvalidInteraction {
                reason: "angle must be finite and within 0 to 180 degrees",
            });
        }
        Ok(Self {
            distance_angstrom,
            angle_degrees,
        })
    }

    /// Source-reported distance in Ångström.
    #[must_use]
    pub const fn distance_angstrom(self) -> f32 {
        self.distance_angstrom
    }

    /// Optional source-reported angle in degrees.
    #[must_use]
    pub const fn angle_degrees(self) -> Option<f32> {
        self.angle_degrees
    }
}

/// Fully resolved glyph presentation, derivable from the scientific inputs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InteractionStyle {
    /// Interaction-class color.
    pub color: Rgba8,
    /// Screen-space mark vocabulary.
    pub pattern: InteractionPattern,
    /// Pixel-stable line width.
    pub width_pixels: f32,
    /// Final alpha before weighted transparency.
    pub opacity: f32,
    /// Repetition period in pixels; ignored by solid lines.
    pub period_pixels: f32,
    /// Fraction of each period occupied by a mark.
    pub duty_cycle: f32,
    /// Optional deterministic presentation phase speed in pixels per frame.
    /// Zero keeps the scientific glyph static.
    pub phase_speed_pixels_per_frame: f32,
}

/// One scientific interaction edge owned by the scene.
#[derive(Clone, PartialEq, Debug)]
pub struct InteractionEdge {
    owner: StructureHandle,
    start: InteractionAnchor,
    end: InteractionAnchor,
    kind: InteractionKind,
    direction: InteractionDirection,
    geometry: InteractionGeometry,
    occupancy: Option<f32>,
    normalized_strength: Option<f32>,
    phase_speed_pixels_per_frame: f32,
    persistence_age_frames: u32,
    persistence_half_life_frames: f32,
    provenance: Arc<str>,
    visible: bool,
}

impl InteractionEdge {
    /// Creates a visible edge from caller-computed facts.
    ///
    /// `provenance` identifies the computation or source dataset; it must not
    /// be empty because the renderer must not present an unexplained claim.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] for coincident anchors or
    /// empty provenance.
    pub fn new(
        owner: StructureHandle,
        start: InteractionAnchor,
        end: InteractionAnchor,
        kind: InteractionKind,
        geometry: InteractionGeometry,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        let provenance = provenance.into();
        if provenance.trim().is_empty() {
            return Err(CoreError::InvalidInteraction {
                reason: "provenance must not be empty",
            });
        }
        if start.position().distance_squared(end.position()) <= f32::EPSILON {
            return Err(CoreError::InvalidInteraction {
                reason: "interaction anchors must not coincide",
            });
        }
        Ok(Self {
            owner,
            start,
            end,
            kind,
            direction: InteractionDirection::Undirected,
            geometry,
            occupancy: None,
            normalized_strength: None,
            phase_speed_pixels_per_frame: 0.0,
            persistence_age_frames: 0,
            persistence_half_life_frames: 0.0,
            provenance,
            visible: true,
        })
    }

    /// Sets source direction without changing endpoint identity.
    #[must_use]
    pub const fn with_direction(mut self, direction: InteractionDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Sets optional occupancy in `[0, 1]`; occupancy maps only to opacity.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] outside the normalized range.
    pub fn with_occupancy(mut self, occupancy: f32) -> Result<Self, CoreError> {
        self.occupancy = Some(normalized(occupancy, "occupancy must be within 0 to 1")?);
        Ok(self)
    }

    /// Sets optional normalized strength in `[0, 1]`; strength maps only to
    /// line width.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] outside the normalized range.
    pub fn with_normalized_strength(mut self, strength: f32) -> Result<Self, CoreError> {
        self.normalized_strength = Some(normalized(
            strength,
            "normalized strength must be within 0 to 1",
        )?);
        Ok(self)
    }

    /// Adds an optional deterministic presentation phase speed in pixels per
    /// frame. It changes only the animated glyph phase, never the source
    /// occupancy or interaction geometry.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] outside `[0, 64]` pixels per
    /// frame.
    pub fn with_phase_speed(mut self, speed: f32) -> Result<Self, CoreError> {
        if !speed.is_finite() || !(0.0..=64.0).contains(&speed) {
            return Err(CoreError::InvalidInteraction {
                reason: "phase speed must be finite and within 0 to 64 pixels per frame",
            });
        }
        self.phase_speed_pixels_per_frame = speed;
        Ok(self)
    }

    /// Supplies caller-computed visual persistence for an interaction that was
    /// last observed some frames ago. A zero half-life disables decay. This is
    /// presentation state only: it never changes the stored occupancy or
    /// claims that the interaction remains scientifically present.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidInteraction`] for a non-finite or negative
    /// half-life, or for a half-life above one million frames.
    pub fn with_persistence(
        mut self,
        age_frames: u32,
        half_life_frames: f32,
    ) -> Result<Self, CoreError> {
        if !half_life_frames.is_finite() || !(0.0..=1_000_000.0).contains(&half_life_frames) {
            return Err(CoreError::InvalidInteraction {
                reason: "persistence half-life must be finite and within 0 to 1000000 frames",
            });
        }
        self.persistence_age_frames = age_frames;
        self.persistence_half_life_frames = half_life_frames;
        Ok(self)
    }

    /// Replaces the caller-computed visual age without changing its half-life.
    pub const fn set_persistence_age(&mut self, age_frames: u32) {
        self.persistence_age_frames = age_frames;
    }

    /// Changes object-level visibility without changing scientific inputs.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Structure used for picking and lifecycle ownership.
    #[must_use]
    pub const fn owner(&self) -> StructureHandle {
        self.owner
    }

    /// First source anchor.
    #[must_use]
    pub const fn start(&self) -> InteractionAnchor {
        self.start
    }

    /// Second source anchor.
    #[must_use]
    pub const fn end(&self) -> InteractionAnchor {
        self.end
    }

    /// Scientific interaction class.
    #[must_use]
    pub const fn kind(&self) -> InteractionKind {
        self.kind
    }

    /// Caller-supplied direction.
    #[must_use]
    pub const fn direction(&self) -> InteractionDirection {
        self.direction
    }

    /// Caller-computed geometry.
    #[must_use]
    pub const fn geometry(&self) -> InteractionGeometry {
        self.geometry
    }

    /// Optional source occupancy.
    #[must_use]
    pub const fn occupancy(&self) -> Option<f32> {
        self.occupancy
    }

    /// Optional normalized source strength.
    #[must_use]
    pub const fn normalized_strength(&self) -> Option<f32> {
        self.normalized_strength
    }

    /// Optional deterministic visual phase speed.
    #[must_use]
    pub const fn phase_speed_pixels_per_frame(&self) -> f32 {
        self.phase_speed_pixels_per_frame
    }

    /// Number of frames since the caller last observed this interaction.
    #[must_use]
    pub const fn persistence_age_frames(&self) -> u32 {
        self.persistence_age_frames
    }

    /// Presentation half-life in frames; zero means no visual decay.
    #[must_use]
    pub const fn persistence_half_life_frames(&self) -> f32 {
        self.persistence_half_life_frames
    }

    /// Source computation or dataset identifier.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Whether this scene object participates in rendering.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Deterministically maps the scientific record to its default glyph.
    ///
    /// Occupancy maps linearly to opacity (`0.35 + 0.65 × occupancy`) and
    /// normalized strength maps linearly to width (`base + 1.5 × strength`).
    /// Both mappings are independently invertible; absent values use the
    /// class default rather than inventing evidence.
    #[must_use]
    pub fn resolved_style(&self) -> InteractionStyle {
        let (color, pattern, base_width, period, duty) = match self.kind {
            InteractionKind::HydrogenBond => (
                Rgba8::opaque(70, 180, 255),
                InteractionPattern::Dashes,
                1.6,
                9.0,
                0.52,
            ),
            InteractionKind::SaltBridge => (
                Rgba8::opaque(238, 90, 210),
                InteractionPattern::Dashes,
                2.0,
                12.0,
                0.68,
            ),
            InteractionKind::PiStacking => (
                Rgba8::opaque(255, 166, 54),
                InteractionPattern::Dots,
                1.9,
                8.0,
                0.34,
            ),
            InteractionKind::Hydrophobic => (
                Rgba8::opaque(230, 205, 72),
                InteractionPattern::Dots,
                1.5,
                7.0,
                0.28,
            ),
            InteractionKind::MetalCoordination => (
                Rgba8::opaque(80, 225, 205),
                InteractionPattern::Solid,
                2.1,
                1.0,
                1.0,
            ),
        };
        let base_opacity = self.occupancy.map_or(0.85, |value| 0.35 + value * 0.65);
        let persistence = if self.persistence_half_life_frames > 0.0 {
            2.0_f32.powf(
                -exact_u32_to_f32(self.persistence_age_frames) / self.persistence_half_life_frames,
            )
        } else {
            1.0
        };
        InteractionStyle {
            color,
            pattern,
            width_pixels: base_width + self.normalized_strength.map_or(0.0, |value| value) * 1.5,
            opacity: base_opacity * persistence,
            period_pixels: period,
            duty_cycle: duty,
            phase_speed_pixels_per_frame: self.phase_speed_pixels_per_frame,
        }
    }
}

fn exact_u32_to_f32(value: u32) -> f32 {
    let high = u16::try_from(value >> 16).map_or(u16::MAX, |part| part);
    let low = u16::try_from(value & u32::from(u16::MAX)).map_or(u16::MAX, |part| part);
    f32::from(high) * 65_536.0 + f32::from(low)
}

fn normalized(value: f32, reason: &'static str) -> Result<f32, CoreError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(CoreError::InvalidInteraction { reason })
    }
}

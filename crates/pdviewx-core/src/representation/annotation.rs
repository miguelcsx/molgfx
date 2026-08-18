//! Persistent caller-authored annotations and measurements.
//!
//! Geometry and values are supplied by `pdbiox` or the caller. The renderer
//! validates, stores and presents them; it never derives domain claims.

use crate::{CoreError, EntityRef, SelectionHandle, StructureHandle};
use pdviewx_math::{Rgba8, Vec3};
use std::sync::Arc;

#[cfg(test)]
#[path = "annotation_tests.rs"]
mod tests;

const MAX_TEXT_BYTES: usize = 4096;

/// A stable world-space anchor with optional bidirectional entity provenance.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AnnotationAnchor {
    position: Vec3,
    entity: Option<EntityRef>,
}

impl AnnotationAnchor {
    /// Creates a free world-space anchor.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for non-finite coordinates.
    pub fn world(position: Vec3) -> Result<Self, CoreError> {
        Self::new(position, None)
    }

    /// Creates a world-space anchor resolving back to one scene entity.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for non-finite coordinates.
    pub fn entity(position: Vec3, entity: EntityRef) -> Result<Self, CoreError> {
        Self::new(position, Some(entity))
    }

    fn new(position: Vec3, entity: Option<EntityRef>) -> Result<Self, CoreError> {
        if !position.is_finite() {
            return Err(invalid("annotation anchor must be finite"));
        }
        Ok(Self { position, entity })
    }

    /// Resolved world-space position in Ångström.
    #[must_use]
    pub const fn position(self) -> Vec3 {
        self.position
    }

    /// Optional entity from which this anchor was resolved.
    #[must_use]
    pub const fn source_entity(self) -> Option<EntityRef> {
        self.entity
    }
}

/// Screen-space marker vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MarkerShape {
    /// Filled circular point.
    #[default]
    Circle,
    /// Four-point diamond.
    Diamond,
    /// Crosshair for precise positions.
    Crosshair,
}

/// Deterministic marker presentation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MarkerStyle {
    /// Marker colour.
    pub color: Rgba8,
    /// Physical-pixel radius.
    pub radius_pixels: f32,
    /// Analytic marker shape.
    pub shape: MarkerShape,
}

impl Default for MarkerStyle {
    fn default() -> Self {
        Self {
            color: Rgba8::opaque(242, 174, 55),
            radius_pixels: 6.0,
            shape: MarkerShape::Circle,
        }
    }
}

/// Human-authored annotation category.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnnotationKind {
    /// Explanatory note.
    Note,
    /// Position marker without prose.
    Marker,
    /// Label attached to a stored selection.
    Region,
    /// Explicit claim to test, visually distinct from a fact.
    Hypothesis,
}

/// One persistent human-authored scene object.
#[derive(Clone, PartialEq, Debug)]
pub struct Annotation {
    owner: StructureHandle,
    kind: AnnotationKind,
    anchor: Option<AnnotationAnchor>,
    region: Option<SelectionHandle>,
    text: Arc<str>,
    marker: MarkerStyle,
    priority: i16,
    visible: bool,
}

impl Annotation {
    /// Creates a note anchored to world/entity geometry.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for empty or oversized text.
    pub fn note(
        owner: StructureHandle,
        anchor: AnnotationAnchor,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Self::anchored(owner, AnnotationKind::Note, anchor, text)
    }

    /// Creates an explicit hypothesis callout.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for empty or oversized text.
    pub fn hypothesis(
        owner: StructureHandle,
        anchor: AnnotationAnchor,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Self::anchored(owner, AnnotationKind::Hypothesis, anchor, text)
    }

    /// Creates a marker with no text label.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for a malformed marker radius.
    pub fn marker(
        owner: StructureHandle,
        anchor: AnnotationAnchor,
        style: MarkerStyle,
    ) -> Result<Self, CoreError> {
        let marker = sanitize_marker(style)?;
        Ok(Self {
            owner,
            kind: AnnotationKind::Marker,
            anchor: Some(anchor),
            region: None,
            text: Arc::from(""),
            marker,
            priority: 0,
            visible: true,
        })
    }

    /// Creates a label attached to one stored selection.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for empty or oversized text.
    pub fn region(
        owner: StructureHandle,
        selection: SelectionHandle,
        label: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            owner,
            kind: AnnotationKind::Region,
            anchor: None,
            region: Some(selection),
            text: text(label)?,
            marker: MarkerStyle::default(),
            priority: 0,
            visible: true,
        })
    }

    fn anchored(
        owner: StructureHandle,
        kind: AnnotationKind,
        anchor: AnnotationAnchor,
        value: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            owner,
            kind,
            anchor: Some(anchor),
            region: None,
            text: text(value)?,
            marker: MarkerStyle::default(),
            priority: 0,
            visible: true,
        })
    }

    /// Sets deterministic decluttering priority. Higher values win.
    #[must_use]
    pub const fn with_priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    /// Changes object visibility without discarding it.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Owning structure used for lifecycle and picking.
    #[must_use]
    pub const fn owner(&self) -> StructureHandle {
        self.owner
    }

    /// Annotation category.
    #[must_use]
    pub const fn kind(&self) -> AnnotationKind {
        self.kind
    }

    /// Optional geometric anchor.
    #[must_use]
    pub const fn anchor(&self) -> Option<AnnotationAnchor> {
        self.anchor
    }

    /// Optional labelled region selection.
    #[must_use]
    pub const fn region_selection(&self) -> Option<SelectionHandle> {
        self.region
    }

    /// Note, hypothesis or region text; empty only for a marker.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Marker presentation used by marker annotations.
    #[must_use]
    pub const fn marker_style(&self) -> MarkerStyle {
        self.marker
    }

    /// Stable decluttering priority.
    #[must_use]
    pub const fn priority(&self) -> i16 {
        self.priority
    }

    /// Whether the object participates in rendering and picking.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Caller-computed measurement type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MeasurementKind {
    /// Two-anchor distance in Ångström.
    Distance,
    /// Three-anchor angle in degrees.
    Angle,
    /// Four-anchor signed dihedral in degrees.
    Dihedral,
}

/// Persistent typed measurement with caller-supplied value and provenance.
#[derive(Clone, PartialEq, Debug)]
pub struct Measurement {
    owner: StructureHandle,
    kind: MeasurementKind,
    anchors: [AnnotationAnchor; 4],
    anchor_count: u8,
    value: f32,
    label: Arc<str>,
    provenance: Arc<str>,
    priority: i16,
    visible: bool,
}

impl Measurement {
    /// Stores a caller-computed two-anchor distance in Ångström.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for invalid values or provenance.
    pub fn distance(
        owner: StructureHandle,
        anchors: [AnnotationAnchor; 2],
        value: f32,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Self::new(
            owner,
            MeasurementKind::Distance,
            &anchors,
            value,
            provenance,
        )
    }

    /// Stores a caller-computed three-anchor angle in degrees.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for invalid values or provenance.
    pub fn angle(
        owner: StructureHandle,
        anchors: [AnnotationAnchor; 3],
        value: f32,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Self::new(owner, MeasurementKind::Angle, &anchors, value, provenance)
    }

    /// Stores a caller-computed four-anchor signed dihedral in degrees.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] for invalid values or provenance.
    pub fn dihedral(
        owner: StructureHandle,
        anchors: [AnnotationAnchor; 4],
        value: f32,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        Self::new(
            owner,
            MeasurementKind::Dihedral,
            &anchors,
            value,
            provenance,
        )
    }

    fn new(
        owner: StructureHandle,
        kind: MeasurementKind,
        source: &[AnnotationAnchor],
        value: f32,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        validate_measurement(kind, value)?;
        let provenance = text(provenance)?;
        let fallback = source[0];
        let mut anchors = [fallback; 4];
        anchors[..source.len()].copy_from_slice(source);
        let label: Arc<str> = match kind {
            MeasurementKind::Distance => Arc::from(format!("{value:.2} Å")),
            MeasurementKind::Angle | MeasurementKind::Dihedral => Arc::from(format!("{value:.1}°")),
        };
        Ok(Self {
            owner,
            kind,
            anchors,
            anchor_count: u8::try_from(source.len()).map_or(4, |count| count),
            value,
            label,
            provenance,
            priority: 0,
            visible: true,
        })
    }

    /// Sets deterministic decluttering priority. Higher values win.
    #[must_use]
    pub const fn with_priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    /// Changes object visibility without discarding it.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Owning structure used for lifecycle and picking.
    #[must_use]
    pub const fn owner(&self) -> StructureHandle {
        self.owner
    }

    /// Measurement type.
    #[must_use]
    pub const fn kind(&self) -> MeasurementKind {
        self.kind
    }

    /// Source anchors in semantic order.
    #[must_use]
    pub fn anchors(&self) -> &[AnnotationAnchor] {
        &self.anchors[..usize::from(self.anchor_count)]
    }

    /// Caller-computed value in the units implied by [`Self::kind`].
    #[must_use]
    pub const fn value(&self) -> f32 {
        self.value
    }

    /// Preformatted deterministic display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Source computation or dataset identifier.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Stable decluttering priority.
    #[must_use]
    pub const fn priority(&self) -> i16 {
        self.priority
    }

    /// Whether the object participates in rendering and picking.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum LabelObject {
    Annotation(Annotation),
    Measurement(Measurement),
}

fn text(value: impl Into<Arc<str>>) -> Result<Arc<str>, CoreError> {
    let value = value.into();
    if value.trim().is_empty() || value.len() > MAX_TEXT_BYTES {
        return Err(invalid("annotation text must contain 1 to 4096 bytes"));
    }
    Ok(value)
}

fn sanitize_marker(style: MarkerStyle) -> Result<MarkerStyle, CoreError> {
    if !style.radius_pixels.is_finite() || !(1.0..=64.0).contains(&style.radius_pixels) {
        return Err(invalid("marker radius must be within 1 to 64 pixels"));
    }
    Ok(style)
}

fn validate_measurement(kind: MeasurementKind, value: f32) -> Result<(), CoreError> {
    let valid = match kind {
        MeasurementKind::Distance => value.is_finite() && value > 0.0,
        MeasurementKind::Angle => value.is_finite() && (0.0..=180.0).contains(&value),
        MeasurementKind::Dihedral => value.is_finite() && (-180.0..=180.0).contains(&value),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(
            "measurement value is outside its physical display domain",
        ))
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidAnnotation { reason }
}

//! Guide-table editing kept separate from the central scene lifecycle.

use crate::{
    CoreError, CrystalCell, EntityKind, EntityRef, Guide, GuideHandle, GuideStyle, PlanarRegion,
    PolylineKind, Scene, StructureHandle, SymmetryInstance,
};

impl Scene {
    /// Samples one cubic Bezier curve into the shared indirect guide batch.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent owner, or
    /// [`CoreError::InvalidAnnotation`] for non-finite control points.
    pub fn add_cubic_bezier(
        &mut self,
        owner: StructureHandle,
        control: [molgfx_math::Vec3; 4],
        segments: u16,
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        if control.iter().any(|point| !point.is_finite()) {
            return Err(CoreError::InvalidAnnotation {
                reason: "Bezier control points must be finite",
            });
        }
        let mut points = Vec::with_capacity(usize::from(segments.max(1)) + 1);
        molgfx_math::sample_cubic_bezier(control, segments, &mut points);
        self.add_polyline(owner, &points, PolylineKind::Open, style)
    }

    /// Adds an open or closed polyline to the shared indirect guide batch.
    ///
    /// Input order is retained. A closed path adds exactly one final segment
    /// from the last point to the first, without duplicating a point record.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent owner, or
    /// [`CoreError::InvalidAnnotation`] for fewer than two points, non-finite
    /// points, repeated consecutive points, or a closed two-point path.
    pub fn add_polyline(
        &mut self,
        owner: StructureHandle,
        points: &[molgfx_math::Vec3],
        kind: PolylineKind,
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        if self.structure(owner).is_none() {
            return Err(CoreError::StaleHandle);
        }
        if points.len() < 2 || (kind == PolylineKind::Closed && points.len() < 3) {
            return Err(CoreError::InvalidAnnotation {
                reason: "polylines require two open points or three closed points",
            });
        }
        let style = style.sanitized();
        for pair in points.windows(2) {
            Guide::new(owner, pair[0], pair[1], style)?;
        }
        if kind == PolylineKind::Closed {
            let (Some(first), Some(last)) = (points.first(), points.last()) else {
                return Err(CoreError::InvalidAnnotation {
                    reason: "a closed polyline needs endpoints",
                });
            };
            Guide::new(owner, *last, *first, style)?;
        }
        let extra = usize::from(kind == PolylineKind::Closed);
        let mut handles = Vec::with_capacity(points.len() - 1 + extra);
        for pair in points.windows(2) {
            handles.push(self.add_guide(Guide::new(owner, pair[0], pair[1], style)?)?);
        }
        if kind == PolylineKind::Closed {
            let (Some(first), Some(last)) = (points.first(), points.last()) else {
                return Err(CoreError::InvalidAnnotation {
                    reason: "a closed polyline needs endpoints",
                });
            };
            handles.push(self.add_guide(Guide::new(owner, *last, *first, style)?)?);
        }
        Ok(handles)
    }

    /// Adds the twelve edges of a caller-supplied unit cell as guides.
    ///
    /// The cell is deliberately lowered to the existing analytic guide table:
    /// it gets the same depth, transparency and picking behaviour without a
    /// second line renderer. Symmetry transforms are applied before storage.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the owner is absent.
    pub fn add_unit_cell(
        &mut self,
        owner: StructureHandle,
        cell: CrystalCell,
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        self.add_edges(owner, cell.edges(), style)
    }

    /// Adds one symmetry-transformed unit-cell instance as twelve guides.
    ///
    /// The transform is applied in Cartesian cell space before the owner's
    /// model-to-world placement, so a caller can add a deterministic assembly
    /// without duplicating the source structure.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the owner is absent.
    pub fn add_unit_cell_instance(
        &mut self,
        owner: StructureHandle,
        cell: CrystalCell,
        instance: SymmetryInstance,
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        self.add_edges(owner, cell.transformed_edges(instance), style)
    }

    /// Adds a rectangular planar-region outline as four analytic guides.
    ///
    /// The outline is deliberately not filled: a future surface/quad path can
    /// add fill without changing the stable guide identity or its picking.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the region owner is absent.
    pub fn add_planar_region(
        &mut self,
        region: PlanarRegion,
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        self.add_edges(region.owner, region.edges(), style)
    }

    /// Adds a caller-integrated streamline as one analytic guide per segment.
    ///
    /// The points are already the result of the caller's vector-field
    /// integration. Lowering them to the existing guide table preserves one
    /// indirect draw path, stable picking and structure placement without
    /// making the renderer a solver.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent owner, or
    /// [`CoreError::InvalidAnnotation`] for fewer than two points, non-finite
    /// points or a repeated consecutive point.
    pub fn add_streamline(
        &mut self,
        owner: StructureHandle,
        points: &[molgfx_math::Vec3],
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        self.add_polyline(owner, points, PolylineKind::Open, style)
    }

    /// Lowers caller-integrated lines to the existing indirect guide batch.
    /// Vector-field integration remains an upstream `pdbiox` operation.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent owner and
    /// [`CoreError::InvalidAnnotation`] when any line has fewer than two points.
    pub fn add_streamline_bundle(
        &mut self,
        owner: StructureHandle,
        lines: &[Vec<molgfx_math::Vec3>],
        style: GuideStyle,
    ) -> Result<Vec<Vec<GuideHandle>>, CoreError> {
        if self.structure(owner).is_none() {
            return Err(CoreError::StaleHandle);
        }
        if lines.iter().any(|line| line.len() < 2) {
            return Err(CoreError::InvalidAnnotation {
                reason: "every streamline seed must produce at least one segment",
            });
        }
        lines
            .iter()
            .map(|line| self.add_streamline(owner, line, style))
            .collect()
    }

    fn add_edges<const N: usize>(
        &mut self,
        owner: StructureHandle,
        edges: [(molgfx_math::Vec3, molgfx_math::Vec3); N],
        style: GuideStyle,
    ) -> Result<Vec<GuideHandle>, CoreError> {
        if self.structure(owner).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let mut handles = Vec::with_capacity(N);
        for (start, end) in edges {
            handles.push(self.add_guide(Guide::new(owner, start, end, style)?)?);
        }
        Ok(handles)
    }

    /// Stores one caller-authored guide.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the owning structure is absent.
    pub fn add_guide(&mut self, guide: Guide) -> Result<GuideHandle, CoreError> {
        if self.structure(guide.owner()).is_none() {
            return Err(CoreError::StaleHandle);
        }
        self.guide_revision = self.guide_revision.wrapping_add(1);
        Ok(GuideHandle(self.guides.insert(guide)))
    }

    /// Resolves a guide handle.
    #[must_use]
    pub fn guide(&self, handle: GuideHandle) -> Option<&Guide> {
        self.guides.get(handle.0)
    }

    /// Resolves a picked guide to its stable handle and caller-authored source.
    ///
    /// Resolution is `O(1)` and rejects a row owned by a different structure.
    #[must_use]
    pub fn guide_for_entity(&self, entity: EntityRef) -> Option<(GuideHandle, &Guide)> {
        if entity.kind != EntityKind::Guide {
            return None;
        }
        let (handle, guide) = self.guides.get_index(entity.index)?;
        if guide.owner() != entity.structure {
            return None;
        }
        Some((GuideHandle(handle), guide))
    }

    /// Mutable resolution; bumps the guide revision because any field edit can
    /// change what draws.
    pub fn guide_mut(&mut self, handle: GuideHandle) -> Option<&mut Guide> {
        let guide = self.guides.get_mut(handle.0)?;
        self.guide_revision = self.guide_revision.wrapping_add(1);
        Some(guide)
    }

    /// Removes a guide, leaving other handles valid.
    pub fn remove_guide(&mut self, handle: GuideHandle) {
        if self.guides.remove(handle.0).is_some() {
            self.guide_revision = self.guide_revision.wrapping_add(1);
        }
    }

    /// Every stored guide in deterministic handle order.
    pub fn guides(&self) -> impl Iterator<Item = (GuideHandle, &Guide)> + '_ {
        self.guides
            .iter()
            .map(|(raw, guide)| (GuideHandle(raw), guide))
    }

    /// Row of a guide in the glyph table, for picking provenance.
    #[must_use]
    pub fn guide_row(handle: GuideHandle) -> u32 {
        handle.row()
    }

    /// Revision of the guide table, bumped by every edit.
    #[must_use]
    pub const fn guide_revision(&self) -> u64 {
        self.guide_revision
    }
}

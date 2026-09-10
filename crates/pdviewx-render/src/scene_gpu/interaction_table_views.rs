// Read-only relation dispatch and draw views.

impl<D: Device> GpuInteractions<D> {
pub(super) fn resolve_dispatches(
    &self,
) -> impl Iterator<Item = RelationResolveDispatch<'_, D>> {
    self.streams.iter().map(|stream| RelationResolveDispatch {
        pipeline: usize::from(stream.pipeline),
        groups: stream.groups,
        group: &stream.group,
    })
}

pub(super) fn cull_dispatches(&self) -> impl Iterator<Item = RelationCullDispatch<'_, D>> {
    self.cull_streams.iter().map(|stream| RelationCullDispatch {
        groups: stream.groups,
        group: &stream.group,
    })
}

pub(super) fn needs_resolve(&self, coordinates_changed: bool) -> bool {
    self.dynamic_dirty
        || (coordinates_changed && self.streams.iter().any(|stream| stream.tracks_coordinates))
}

pub(super) fn mark_resolved(&mut self) {
    self.dynamic_dirty = false;
}

pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
    if self.count == 0 {
        return None;
    }
    Some((self.render_group.as_ref()?, self.args.as_ref()?))
}

pub(super) const fn has_visible(&self) -> bool {
    self.count > 0
}
}

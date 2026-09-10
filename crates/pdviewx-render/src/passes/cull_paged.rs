fn record_paged<D: Device, P: ComputePassEncoder<D>>(
    pass: &mut P,
    scene: &crate::scene_gpu::GpuScene<D>,
    cull: &CullPass<D>,
) {
    if let Some((group, dispatch)) = scene.paged_chunk_cull() {
        pass.set_bind_group(0, &scene.group0, &[]);
        pass.set_bind_group(1, group, &[]);
        pass.set_pipeline(&cull.paged_reset);
        pass.dispatch(2, 1, 1);
        pass.set_pipeline(&cull.paged_cull);
        pass.dispatch(dispatch[0], dispatch[1], dispatch[2]);
    }
    if let Some((group, dispatch)) = scene.paged_bond_cull() {
        pass.set_bind_group(0, &scene.group0, &[]);
        pass.set_bind_group(1, group, &[]);
        pass.set_pipeline(&cull.paged_bond_reset);
        pass.dispatch(1, 1, 1);
        pass.set_pipeline(&cull.paged_bond_cull);
        pass.dispatch(dispatch[0], dispatch[1], dispatch[2]);
    }
}

// Stable relation keys and limits.

fn table_chunk(row: u32, generation: u32) -> ChunkId {
    ChunkId::new(u64::from(row) | (u64::from(generation) << 32))
}

fn source_binding_revision<D: Device>(structures: &[GpuStructure<D>]) -> u64 {
    structures
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, source| {
            hash.wrapping_mul(0x0000_0100_0000_01b3)
                ^ u64::from(source.handle.row())
                ^ (u64::from(source.handle.generation()) << 32)
                ^ source.binding_revision
        })
}

fn row_limit() -> pdviewx_gpu::GpuError {
    pdviewx_gpu::GpuError::LimitExceeded {
        resource: "generic relation rows",
        limit: u64::from(u32::MAX),
    }
}

fn missing_domain(domain: Option<RowDomain>) -> RenderError {
    let Some(domain) = domain else {
        return RenderError::PickingOwnerMissing;
    };
    RenderError::RelationSourceMissing { domain }
}

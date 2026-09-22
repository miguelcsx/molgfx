pub(super) fn representation_layout<D: Device>(
    device: &D,
) -> Result<D::BindGroupLayout, RenderError> {
    let entries = representation_entries();
    validate_storage_limit(
        "group2: per-representation",
        &entries,
        device.capabilities().max_storage_buffers_per_shader_stage,
    )?;
    Ok(device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: per-representation",
        entries: &entries,
    }))
}

fn representation_entries() -> [BindGroupLayoutEntry; 20] {
    [
        storage_visible(0, all_stages()),
        storage_visible(1, all_stages()),
        BindGroupLayoutEntry {
            binding: 2,
            visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
            ty: BindingType::Uniform,
        },
        storage_visible(3, ShaderStages::VERTEX),
        storage_visible(4, ShaderStages::VERTEX),
        storage_visible(5, ShaderStages::VERTEX),
        // The surface's vertex stage reads the BVH for the boundary it
        // projects, so this is visible everywhere a molecular surface is.
        storage_visible(6, all_stages()),
        storage_visible(7, ShaderStages::FRAGMENT.union(ShaderStages::COMPUTE)),
        storage_visible(8, ShaderStages::FRAGMENT.union(ShaderStages::COMPUTE)),
        BindGroupLayoutEntry {
            binding: 9,
            visibility: all_stages(),
            ty: BindingType::Uniform,
        },
        BindGroupLayoutEntry {
            binding: 10,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture3dFloat { filterable: false },
        },
        BindGroupLayoutEntry {
            binding: 11,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture3dFloat { filterable: false },
        },
        BindGroupLayoutEntry {
            binding: 12,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture3dFloat { filterable: false },
        },
        storage_visible(13, ShaderStages::VERTEX.union(ShaderStages::FRAGMENT)),
        BindGroupLayoutEntry {
            binding: 14,
            visibility: all_stages(),
            ty: BindingType::Uniform,
        },
        storage_visible(16, all_stages()),
        BindGroupLayoutEntry {
            binding: 17,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        // The colour scheme block. The vertex stage resolves an atom's colour —
        // it writes the per-instance payload the fragment stage shades — so this
        // is visible to every stage that draws an atom.
        BindGroupLayoutEntry {
            binding: 18,
            visibility: all_stages(),
            ty: BindingType::Uniform,
        },
        storage_visible(19, ShaderStages::FRAGMENT),
        BindGroupLayoutEntry {
            binding: 20,
            visibility: all_stages(),
            ty: BindingType::Uniform,
        },
    ]
}

pub(super) fn quality_layout<D: Device>(device: &D) -> Result<D::BindGroupLayout, RenderError> {
    let entries = quality_entries();
    validate_storage_limit(
        "group2: quality tracing",
        &entries,
        device.capabilities().max_storage_buffers_per_shader_stage,
    )?;
    Ok(device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: quality tracing",
        entries: &entries,
    }))
}

fn quality_entries() -> [BindGroupLayoutEntry; 14] {
    [
        storage_visible(0, ShaderStages::FRAGMENT),
        storage_visible(1, ShaderStages::FRAGMENT),
        BindGroupLayoutEntry {
            binding: 2,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        storage_visible(6, ShaderStages::FRAGMENT),
        storage_visible(7, ShaderStages::FRAGMENT),
        storage_visible(8, ShaderStages::FRAGMENT),
        BindGroupLayoutEntry {
            binding: 9,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        BindGroupLayoutEntry {
            binding: 14,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        storage_visible(16, ShaderStages::FRAGMENT),
        BindGroupLayoutEntry {
            binding: 17,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        // The colour scheme block. A quality frame binds the same group2 as a
        // raster frame, so this layout declares exactly what that group
        // carries, at the visibility the group's own layout uses.
        BindGroupLayoutEntry {
            binding: 18,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        storage_visible(19, ShaderStages::FRAGMENT),
        BindGroupLayoutEntry {
            binding: 20,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Uniform,
        },
        storage_visible(21, ShaderStages::FRAGMENT),
    ]
}

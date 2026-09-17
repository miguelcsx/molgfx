// Shared portable binding constructors.

pub(super) fn occupancy_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "temporal occupancy accumulation",
        entries: &[
            writable_storage(0),
            storage_visible(1, ShaderStages::COMPUTE),
            storage_visible(2, ShaderStages::COMPUTE),
            compute_uniform(3),
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture3dWrite {
                    format: molgfx_gpu::TextureFormat::R32Float,
                },
            },
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture3dWrite {
                    format: molgfx_gpu::TextureFormat::Rg32Float,
                },
            },
        ],
    })
}

fn relation_cull_entries() -> [BindGroupLayoutEntry; 10] {
    [
        storage_visible(0, ShaderStages::COMPUTE),
        writable_storage(1),
        writable_storage(2),
        writable_storage(3),
        compute_uniform(4),
        storage_visible(5, ShaderStages::COMPUTE),
        storage_visible(6, ShaderStages::COMPUTE),
        storage_visible(7, ShaderStages::COMPUTE),
        writable_storage(8),
        compute_uniform(9),
    ]
}

const fn compute_uniform(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Uniform,
    }
}

pub(super) fn storage(binding: u32) -> BindGroupLayoutEntry {
    storage_visible(binding, all_stages())
}

pub(super) fn writable_storage(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Storage { read_only: false },
    }
}

fn storage_visible(binding: u32, visibility: ShaderStages) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Storage { read_only: true },
    }
}

fn all_stages() -> ShaderStages {
    ShaderStages::VERTEX
        .union(ShaderStages::FRAGMENT)
        .union(ShaderStages::COMPUTE)
}

//! Persistent scene bind-group layouts, grouped away from synchronization.

use pdviewx_gpu::{BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, Device, ShaderStages};

pub(super) fn volume_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: density volume representation",
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture3dFloat { filterable: false },
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture3dFloat { filterable: false },
            },
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture3dFloat { filterable: false },
            },
        ],
    })
}

pub(super) fn segmentation_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: categorical segmentation representation",
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture3dUint,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Storage { read_only: true },
            },
        ],
    })
}

pub(super) fn interaction_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: interaction glyph table",
        entries: &[storage(0)],
    })
}

pub(super) fn primitive_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: primitive table",
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: all_stages(),
                ty: BindingType::Storage { read_only: true },
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Storage { read_only: true },
            },
            storage_visible(2, ShaderStages::COMPUTE),
        ],
    })
}

pub(super) fn primitive_shadow_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    // The shadow module shares one binding space with the atom caster, which
    // owns bindings 0..5 and 13, so the primitive buffer is read at binding 6.
    // It is the same PrimitiveGpu records the gbuffer pass draws, bound here at
    // a slot that does not collide with the atom caster's storage.
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: primitive shadow casters",
        entries: &[storage_visible(
            6,
            ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
        )],
    })
}

pub(super) fn primitive_motion_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: primitive particle motion",
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Storage { read_only: false },
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Storage { read_only: false },
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Storage { read_only: true },
            },
        ],
    })
}

pub(super) fn label_render_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: visible semantic labels",
        entries: &[storage_visible(0, ShaderStages::VERTEX)],
    })
}

pub(super) fn overlay_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: screen overlays",
        entries: &[storage_visible(0, ShaderStages::VERTEX)],
    })
}

pub(super) fn label_declutter_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: deterministic label decluttering",
        entries: &[
            storage_visible(0, ShaderStages::COMPUTE),
            storage_visible(1, ShaderStages::COMPUTE),
            writable_storage(2),
            writable_storage(3),
            writable_storage(4),
            compute_uniform(5),
        ],
    })
}

pub(super) fn representation_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: per-representation",
        entries: &[
            storage_visible(0, all_stages()),
            storage_visible(1, all_stages()),
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            storage_visible(3, ShaderStages::VERTEX.union(ShaderStages::FRAGMENT)),
            storage_visible(4, ShaderStages::VERTEX),
            storage_visible(5, ShaderStages::VERTEX),
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
                ty: BindingType::Texture3dUint,
            },
            BindGroupLayoutEntry {
                binding: 12,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture3dFloat { filterable: false },
            },
            storage_visible(13, all_stages()),
            BindGroupLayoutEntry {
                binding: 14,
                visibility: all_stages(),
                ty: BindingType::Uniform,
            },
            storage_visible(15, ShaderStages::FRAGMENT.union(ShaderStages::COMPUTE)),
        ],
    })
}

pub(super) fn cartoon_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: cartoon representation",
        entries: &[
            storage(0),
            storage(1),
            BindGroupLayoutEntry {
                binding: 2,
                // The model transform is used per vertex and the structure id
                // it carries is written per fragment, so both stages bind it.
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            BindGroupLayoutEntry {
                binding: 3,
                // The clip planes are evaluated per vertex and the ribbon
                // material is read per fragment, so both stages bind it.
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
        ],
    })
}

pub(super) fn cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "cull slot",
        entries: &[
            storage(0),
            storage(1),
            writable_storage(2),
            writable_storage(3),
            writable_storage(4),
            writable_storage(5),
            compute_uniform(6),
            compute_uniform(7),
            compute_uniform(8),
            storage_visible(9, ShaderStages::COMPUTE),
            writable_storage(10),
        ],
    })
}

pub(super) fn trajectory_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "trajectory interpolation",
        entries: &[
            storage(0),
            storage(1),
            writable_storage(2),
            writable_storage(3),
            compute_uniform(4),
        ],
    })
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

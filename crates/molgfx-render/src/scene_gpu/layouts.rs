//! Persistent scene bind-group layouts, grouped away from synchronization.

use crate::error::RenderError;
use molgfx_gpu::{BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, Device, ShaderStages};

#[path = "layouts/storage.rs"]
mod storage_limits;
#[cfg(test)]
use storage_limits::storage_counts;
use storage_limits::validate_storage_limit;

#[cfg(test)]
#[path = "layouts_tests.rs"]
mod tests;

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
            storage_visible(3, ShaderStages::FRAGMENT),
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Uniform,
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
            storage_visible(3, ShaderStages::FRAGMENT),
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Uniform,
            },
        ],
    })
}

pub(super) fn interaction_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: interaction glyph table",
        entries: &[storage(0), storage_visible(1, ShaderStages::VERTEX)],
    })
}

pub(super) fn relation_cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group1: relation culling",
        entries: &relation_cull_entries(),
    })
}

pub(super) fn relation_resolve_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group1: dynamic relation resolver",
        entries: &relation_resolve_entries(),
    })
}

fn relation_resolve_entries() -> [BindGroupLayoutEntry; 10] {
    [
        storage_visible(0, ShaderStages::COMPUTE),
        storage_visible(1, ShaderStages::COMPUTE),
        storage_visible(2, ShaderStages::COMPUTE),
        writable_storage(3),
        compute_uniform(4),
        compute_uniform(5),
        storage_visible(6, ShaderStages::COMPUTE),
        storage_visible(7, ShaderStages::COMPUTE),
        compute_uniform(8),
        compute_uniform(9),
    ]
}

pub(super) fn generic_point_cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group1: generic point culling",
        entries: &[
            storage_visible(0, ShaderStages::COMPUTE),
            writable_storage(1),
            compute_uniform(3),
            writable_storage(4),
            storage_visible(5, ShaderStages::COMPUTE),
            storage_visible(6, ShaderStages::COMPUTE),
            storage_visible(7, ShaderStages::COMPUTE),
            writable_storage(8),
            compute_uniform(9),
            storage_visible(10, ShaderStages::COMPUTE),
        ],
    })
}

pub(super) fn generic_point_render_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: generic point rendering",
        entries: &[
            storage_visible(0, ShaderStages::VERTEX),
            storage_visible(1, ShaderStages::VERTEX),
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            storage_visible(3, ShaderStages::FRAGMENT),
            storage_visible(4, ShaderStages::VERTEX.union(ShaderStages::FRAGMENT)),
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            BindGroupLayoutEntry {
                binding: 6,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Uniform,
            },
            storage_visible(7, ShaderStages::VERTEX),
        ],
    })
}

pub(super) fn generic_instance_cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group1: generic instance culling",
        entries: &[
            storage_visible(0, ShaderStages::COMPUTE),
            writable_storage(1),
            compute_uniform(3),
            storage_visible(4, ShaderStages::COMPUTE),
            storage_visible(5, ShaderStages::COMPUTE),
            storage_visible(6, ShaderStages::COMPUTE),
            writable_storage(7),
            compute_uniform(8),
            storage_visible(9, ShaderStages::COMPUTE),
            storage_visible(10, ShaderStages::COMPUTE),
        ],
    })
}

pub(super) fn generic_instance_render_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: generic analytic instances",
        entries: &[
            storage_visible(5, ShaderStages::VERTEX),
            storage_visible(6, ShaderStages::VERTEX),
            storage_visible(7, ShaderStages::VERTEX),
            storage_visible(8, ShaderStages::VERTEX),
            BindGroupLayoutEntry {
                binding: 9,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            storage_visible(10, ShaderStages::FRAGMENT),
            storage_visible(11, ShaderStages::VERTEX.union(ShaderStages::FRAGMENT)),
            BindGroupLayoutEntry {
                binding: 12,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            BindGroupLayoutEntry {
                binding: 13,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Uniform,
            },
            storage_visible(14, ShaderStages::VERTEX),
            storage_visible(15, ShaderStages::VERTEX),
        ],
    })
}

pub(super) fn instance_timeline_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group0: generic instance timeline",
        entries: &[
            storage_visible(0, ShaderStages::COMPUTE),
            storage_visible(1, ShaderStages::COMPUTE),
            writable_storage(2),
            compute_uniform(3),
        ],
    })
}

pub(super) fn attribute_timeline_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "attribute timeline materialization",
        entries: &[writable_storage(0), compute_uniform(1)],
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

pub(super) fn ligand_pose_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "group2: compact ligand poses",
        entries: &[
            storage_visible(7, ShaderStages::VERTEX),
            storage_visible(8, ShaderStages::VERTEX),
            storage_visible(9, ShaderStages::VERTEX),
            storage_visible(10, ShaderStages::VERTEX),
            storage_visible(11, ShaderStages::VERTEX),
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

include!("layouts/representation.rs");

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
            storage_visible(4, ShaderStages::VERTEX),
            storage_visible(5, ShaderStages::VERTEX),
            storage_visible(6, ShaderStages::VERTEX),
            storage_visible(7, ShaderStages::VERTEX),
            storage_visible(8, ShaderStages::VERTEX.union(ShaderStages::FRAGMENT)),
            storage_visible(9, ShaderStages::FRAGMENT),
            storage_visible(10, ShaderStages::FRAGMENT),
            storage_visible(11, ShaderStages::FRAGMENT),
            BindGroupLayoutEntry {
                binding: 12,
                visibility: ShaderStages::VERTEX.union(ShaderStages::FRAGMENT),
                ty: BindingType::Uniform,
            },
            storage_visible(13, ShaderStages::VERTEX),
        ],
    })
}

pub(super) fn atom_cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    let entries = atom_cull_entries();
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "atom cull slot",
        entries: &entries,
    })
}

fn atom_cull_entries() -> [BindGroupLayoutEntry; 11] {
    [
        storage_visible(0, ShaderStages::COMPUTE),
        writable_storage(2),
        writable_storage(4),
        writable_storage(5),
        compute_uniform(6),
        compute_uniform(7),
        compute_uniform(8),
        storage_visible(9, ShaderStages::COMPUTE),
        writable_storage(10),
        writable_storage(14),
        compute_uniform(15),
    ]
}

pub(super) fn bond_cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    let entries = bond_cull_entries();
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "bond cull slot",
        entries: &entries,
    })
}

fn bond_cull_entries() -> [BindGroupLayoutEntry; 12] {
    [
        storage_visible(0, ShaderStages::COMPUTE),
        storage_visible(1, ShaderStages::COMPUTE),
        writable_storage(2),
        writable_storage(3),
        writable_storage(4),
        writable_storage(5),
        compute_uniform(6),
        compute_uniform(7),
        compute_uniform(8),
        storage_visible(9, ShaderStages::COMPUTE),
        writable_storage(14),
        compute_uniform(15),
    ]
}

pub(super) fn visual_cull_layout<D: Device>(device: &D) -> D::BindGroupLayout {
    let entries = visual_cull_entries();
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "visual evaluation slot",
        entries: &entries,
    })
}

fn visual_cull_entries() -> [BindGroupLayoutEntry; 10] {
    [
        storage_visible(0, ShaderStages::COMPUTE),
        writable_storage(2),
        writable_storage(4),
        compute_uniform(8),
        storage_visible(9, ShaderStages::COMPUTE),
        storage_visible(11, ShaderStages::COMPUTE),
        storage_visible(12, ShaderStages::COMPUTE),
        storage_visible(13, ShaderStages::COMPUTE),
        writable_storage(14),
        compute_uniform(15),
    ]
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

include!("layouts_helpers.rs");

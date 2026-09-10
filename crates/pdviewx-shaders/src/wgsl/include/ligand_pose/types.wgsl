// Compact reusable-topology ligand pose tables.

const LIGAND_POSE_SPHERE: u32 = 0u;
const LIGAND_POSE_CAPSULE: u32 = 3u;

override LIGAND_POSE_SHAPE: u32 = LIGAND_POSE_SPHERE;
override LIGAND_POSE_TRANSLUCENT: u32 = 0u;

struct LigandPoseTransformGpu {
    translation_opacity: vec4f,
    orientation: vec4f,
}

struct LigandPoseBondGpu {
    center_length: vec4f,
    orientation: vec4f,
}

struct LigandPoseBatchGpu {
    topology: vec4u,
    poses: vec4u,
    sampling: vec4u,
    radii: vec4f,
    model: mat4x4f,
}

@group(2) @binding(7)
var<storage, read> ligand_pose_atoms: array<vec4f>;

@group(2) @binding(8)
var<storage, read> ligand_pose_bonds: array<LigandPoseBondGpu>;

@group(2) @binding(9)
var<storage, read> ligand_pose_transforms: array<LigandPoseTransformGpu>;

@group(2) @binding(10)
var<storage, read> ligand_pose_styles: array<u32>;

@group(2) @binding(11)
var<storage, read> ligand_pose_batches: array<LigandPoseBatchGpu>;

struct LigandPoseInstance {
    center_radius: vec4f,
    orientation: vec4f,
    size: vec3f,
    color: vec4f,
    metadata: vec4u,
}

//! Where the arena places each attribute table.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::scene_gpu) struct AttributeArenaBinding {
    pub(in crate::scene_gpu) offsets: [u32; 4],
    pub(in crate::scene_gpu) layouts: [u32; 4],
}

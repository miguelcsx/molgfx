//! World-space bounds over the scene's heterogeneous sources.

use super::Scene;
use pdviewx_math::{Aabb, Vec3};

impl Scene {
    /// The world-space bound over every placed structure, `O(atoms)`.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        let mut aabb = Aabb::EMPTY;
        for (_, placed) in self.structures.iter() {
            aabb = aabb.union(&placed.world_aabb());
        }
        for (_, volume) in self.volumes.iter() {
            aabb = aabb.union(&volume.world_aabb(self));
        }
        for (_, volume) in self.segmentations.iter() {
            aabb = aabb.union(&volume.value.world_aabb());
        }
        for (_, interaction) in self.interactions.iter() {
            aabb.extend(interaction.start().position());
            aabb.extend(interaction.end().position());
        }
        for (_, guide) in self.guides.iter() {
            if let Some(placed) = self.structure(guide.owner()) {
                aabb.extend(placed.model_to_world.transform_point3(guide.start()));
                aabb.extend(placed.model_to_world.transform_point3(guide.end()));
            }
        }
        for (_, primitive) in self.primitive.iter() {
            if !primitive.visible() {
                continue;
            }
            if let Some(placed) = self.structure(primitive.owner()) {
                let transform = placed.model_to_world;
                for corner in primitive.bounds().corners() {
                    aabb.extend(transform.transform_point3(corner));
                }
                if let crate::Primitive::Particle(value) = primitive
                    && let Some(motion) = value.motion
                {
                    let margin = Vec3::splat(value.size.max_element() * 0.5);
                    let motion_bounds = pdviewx_math::Aabb::new(
                        motion.bounds().min - margin,
                        motion.bounds().max + margin,
                    );
                    for corner in motion_bounds.corners() {
                        aabb.extend(transform.transform_point3(corner));
                    }
                }
            }
        }
        for (_, batch) in self.ligand_pose_batches.iter() {
            if !batch.visible() {
                continue;
            }
            let Some(placed) = self.structure(batch.owner()) else {
                continue;
            };
            for corner in batch.bounds().corners() {
                aabb.extend(placed.model_to_world.transform_point3(corner));
            }
        }
        for (_, batch) in self.point_batches.iter() {
            if batch.visible() {
                aabb = aabb.union(&batch.bounds());
            }
        }
        for (_, batch) in self.instance_batches.iter() {
            if batch.visible() {
                aabb = aabb.union(&batch.bounds());
            }
        }
        aabb
    }
}

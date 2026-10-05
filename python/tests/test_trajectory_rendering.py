"""Resident trajectory lifecycle through the public Python renderer."""

import json
import unittest

import molframe

import molgfx


class TrajectoryRenderingTests(unittest.TestCase):
    """Removing metadata restores source coordinates; adding it restores playback."""

    def test_a_trajectory_can_be_removed_and_reactivated_on_the_same_renderer(self) -> None:
        source = molframe.read(
            b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \n"
            b"ATOM      2  CA  ALA A   2       4.000   0.000   0.000  1.00 10.00           C  \n"
            b"END\n",
            name="trajectory.pdb",
        )
        scene = molgfx.Scene(source)
        scene.add(molgfx.rep.spacefill(target="all"))
        scene.set_camera(scene.frame("all"))
        # Hold quality fixed across trajectory edits so adaptation cannot change the image.
        profile = molgfx.profile.highest_fixed(60).with_effect(molgfx.effect.motion_blur())
        renderer = molgfx.Renderer(profile=profile)
        reference = renderer.render_image(scene, size=(128, 128)).pixels()
        descriptor = molgfx.trajectory.bind(
            structure=scene.structure_id,
            source=molgfx.data.source("resident-pair"),
            frame_count=2,
        )
        identity = scene.add(descriptor)
        scene.bind_trajectory(
            source_hash="resident-pair",
            start=molgfx.trajectory.frame(0, 0.0, ((0.0, 0.0, 0.0), (4.0, 0.0, 0.0))),
            end=molgfx.trajectory.frame(1, 1.0, ((0.0, 3.0, 0.0), (4.0, -3.0, 0.0))),
            sample_time=0.25,
        )
        moved = renderer.render_image(scene, size=(128, 128)).pixels()
        self.assertNotEqual(reference, moved)
        scene.apply(
            molgfx.ScenePatch(
                json.dumps(
                    {
                        "base_revision": scene.revision,
                        "operations": [{"op": "remove_trajectory", "id": identity.value}],
                    }
                )
            )
        )
        self.assertEqual(reference, renderer.render_image(scene, size=(128, 128)).pixels())
        scene.add(descriptor)
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.75)
        reactivated = renderer.render_image(scene, size=(128, 128)).pixels()
        self.assertNotEqual(reference, reactivated)
        self.assertNotEqual(moved, reactivated)
        self.assertEqual(reactivated, renderer.render_image(scene, size=(128, 128)).pixels())
        fresh = molgfx.Renderer(profile=profile)
        self.assertEqual(reactivated, fresh.render_image(scene, size=(128, 128)).pixels())

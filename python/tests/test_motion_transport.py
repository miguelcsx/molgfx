"""Coordinate samples cross the widget without authoring edits or resending frame columns."""

import json
import struct
import unittest

import molframe
from test_hydrogen_bonds import PDB

import molgfx


class MotionTransportTests(unittest.TestCase):
    """Exercise live callbacks, immutable column owners and scientific pose metadata."""

    def test_interval_swaps_reuse_shared_frames_without_changing_authored_history(self) -> None:
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        viewer = molgfx.Viewer(scene)
        self.addCleanup(viewer.close)
        start = molgfx.trajectory.frame(7, 7, ((0, 0, 0), (1, 0, 0), (2.8, 0, 0)))
        middle = molgfx.trajectory.frame(8, 8, ((0, 2, 0), (1, 2, 0), (2.8, 2, 0)))
        end = molgfx.trajectory.frame(9, 9, ((0, 4, 0), (1, 4, 0), (2.8, 4, 0)))
        authored = scene.revision, viewer.scene_spec, viewer.patch_sequence
        structure_bytes = viewer.structure_payloads[0]
        scene.set_trajectory_pair(
            structure=scene.structure_id, start=start, end=middle, sample_time=7.25
        )
        self.assertEqual(viewer.trajectory_bindings, [])
        self.assertEqual(viewer.playback_pair, viewer.playback_bindings[0])
        columns = {frame[0]: frame[3] for frame in viewer.coordinate_frames}
        shared = viewer.playback_pair[2]
        scene.set_trajectory_pair(
            structure=scene.structure_id, start=middle, end=end, sample_time=8.25
        )
        self.assertEqual(viewer.playback_pair[1], shared)
        self.assertEqual(len(viewer.coordinate_frames), 2)
        self.assertIs(
            {frame[0]: frame[3] for frame in viewer.coordinate_frames}[shared],
            columns[shared],
        )
        self.assertEqual((scene.revision, viewer.scene_spec, viewer.patch_sequence), authored)
        self.assertIs(viewer.structure_payloads[0], structure_bytes)
        pair = viewer.playback_pair
        scene.set_trajectory_pair(
            structure=scene.structure_id, start=middle, end=end, sample_time=8.75
        )
        self.assertEqual(viewer.playback_pair, pair)
        self.assertEqual(viewer.trajectory_sample, (str(scene.structure_id.value), 8.75))
        before = scene.browser_playback_bindings()
        with self.assertRaises(molgfx.MolgfxError):
            scene.set_trajectory_pair(
                structure=scene.structure_id, start=start, end=end, sample_time=10
            )
        self.assertEqual(scene.browser_playback_bindings(), before)
        self.assertEqual(viewer.playback_pair, pair)
        viewer.sync_request += 1
        self.assertEqual(viewer.playback_bindings[0][-1], 8.75)
        self.assertEqual(scene.revision, authored[0])
        scene.set_camera(None)
        self.assertEqual(viewer.patch_sequence, authored[2] + 1)
        self.assertEqual(viewer.error, "")

    def test_resident_samples_preserve_revision_bytes_and_the_authored_patch_stream(self) -> None:
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        identity = scene.add(
            molgfx.trajectory.bind(
                structure=scene.structure_id,
                source=molgfx.data.source("widget-frames"),
                frame_count=2,
            )
        )
        viewer = molgfx.Viewer(scene)
        self.addCleanup(viewer.close)
        scene.bind_trajectory(
            source_hash="widget-frames",
            start=molgfx.trajectory.frame(0, 0.0, ((0, 0, 0), (1, 0, 0), (2.8, 0, 0))),
            end=molgfx.trajectory.frame(1, 1.0, ((0, 2, 0), (1, 2, 0), (2.8, 2, 0))),
            sample_time=0.25,
        )
        self.assertEqual(viewer.trajectory_bindings[0][0], str(identity.value))
        self.assertEqual(len(viewer.coordinate_frames), 2)
        frames, intervals = scene.browser_motion_bindings()
        start, end = intervals[0][1:3]
        columns = {frame[0]: frame[3] for frame in frames}
        self.assertEqual(struct.unpack("<9f", columns[start])[:3], (0, 0, 0))
        self.assertEqual(struct.unpack("<9f", columns[end])[:3], (0, 2, 0))
        revision, snapshot, sequence = scene.revision, viewer.scene_spec, viewer.patch_sequence
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.75)
        self.assertEqual(viewer.trajectory_sample, (str(scene.structure_id.value), 0.75))
        self.assertEqual(
            (scene.revision, viewer.scene_spec, viewer.patch_sequence),
            (revision, snapshot, sequence),
        )
        after, intervals = scene.browser_motion_bindings()
        self.assertEqual(intervals[0][-1], 0.75)
        for frame in after:
            self.assertIs(frame[3], columns[frame[0]])
        with self.assertRaises(molgfx.MolgfxError):
            scene.set_trajectory_time(structure=scene.structure_id, seconds=2.0)
        self.assertEqual(viewer.trajectory_sample[-1], 0.75)
        viewer.sync_request += 1
        self.assertEqual(viewer.trajectory_bindings[0][-1], 0.75)
        scene.set_camera(None)
        self.assertEqual(viewer.patch_sequence, sequence + 1)
        self.assertEqual(viewer.error, "")
        viewer.close()
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.5)
        self.assertEqual(viewer.trajectory_sample[-1], 0.75)

    def test_scientific_pose_reuses_resident_frames_and_refuses_stale_publication(self) -> None:
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        scene.add(
            molgfx.trajectory.bind(
                structure=scene.structure_id,
                source=molgfx.data.source("surface-frames"),
                frame_count=2,
            )
        )
        scene.bind_trajectory(
            source_hash="surface-frames",
            start=molgfx.trajectory.frame(0, 0, ((0, 0, 0), (1, 0, 0), (2.8, 0, 0))),
            end=molgfx.trajectory.frame(1, 1, ((0, 2, 0), (1, 2, 0), (2.8, 2, 0))),
            sample_time=0.25,
        )
        pose = scene.molecular_pose(structure=scene.structure_id)
        self.assertEqual(pose.interpolation, 0.25)
        identity = scene.add(
            molgfx.rep.surf(target="all", source=molgfx.data.source("sampled-surface"))
        )
        # Small provider-shaped geometry isolates transport and pose validation.
        data = json.dumps(
            {
                "method": "surf",
                "controls": {"probe_radius": 1.4, "density": 3.0, "all_components": True},
                "mesh": {
                    "vertices": [[0, 0.5, 0], [1, 0.5, 0], [0, 1.5, 0]],
                    "normals": [[0, 0, 1]] * 3,
                    "faces": [[0, 1, 2]],
                    "atom_rows": [0, 1, 2],
                    "discarded_degenerate_faces": 0,
                },
            }
        )
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.5)
        before = scene.to_json()
        with self.assertRaises(molgfx.MolgfxError):
            scene.bind_molecular_surface(identity, data, pose=pose)
        self.assertEqual(scene.to_json(), before)
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.25)
        scene.bind_molecular_surface(identity, data, pose=pose)
        metadata = json.loads(scene.browser_bindings()[2][0][1])
        frames, intervals = scene.browser_motion_bindings()
        self.assertEqual(metadata[3], list(intervals[0][1:]))
        self.assertEqual(len(frames), 2)
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.5)
        self.assertEqual(json.loads(scene.browser_bindings()[2][0][1])[3][-1], 0.25)
        self.assertEqual(scene.browser_motion_bindings()[1][0][-1], 0.5)

"""Bulk classified hydrogen bonds retain atomic addresses and binding atomicity."""

import json
import unittest

import molframe

import molgfx

PDB = b"""ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N
ATOM      2  H   ALA A   1       1.000   0.000   0.000  1.00 20.00           H
ATOM      3  O   ALA A   2       2.800   0.000   0.000  1.00 20.00           O
END
"""


class HydrogenBondTests(unittest.TestCase):
    """The renderer consumes chemical results and follows their source atoms."""

    def test_a_failed_table_binding_keeps_the_descriptor_and_can_be_corrected(self) -> None:
        """An invalid row cannot poison the content identity needed by a valid retry."""
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        identity = scene.add(
            molgfx.interaction.hydrogen_bonds(
                structure=scene.structure_id,
                source=molgfx.data.source("classified-explicit-hydrogen-table"),
                target="not element H",
            )
        )
        before = scene.spec.to_json()
        self.assertEqual(len(json.loads(scene.unresolved_overlays())), 1)
        with self.assertRaises(molgfx.SpecError):
            scene.bind_hydrogen_bonds(identity, [(0, 1, 3)])
        self.assertEqual(scene.spec.to_json(), before)
        scene.bind_hydrogen_bonds(identity, [(0, 1, 2)])
        self.assertEqual(json.loads(scene.unresolved_overlays()), [])
        descriptor = json.loads(scene.spec.to_json())["interactions"][str(identity.value)]
        self.assertEqual(descriptor["mode"], "hydrogen_bonds")

    def test_candidates_require_exact_geometry_and_a_sufficient_distance_envelope(self) -> None:
        """Failed candidate bindings preserve the geometric policy and allow retry."""
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        identity = scene.add(
            molgfx.interaction.hydrogen_bonds(
                structure=scene.structure_id,
                source=molgfx.data.source("oriented-motion-candidates"),
                geometry=(3.5, 150.0, 2.5),
            )
        )
        before = scene.spec.to_json()
        with self.assertRaises(molgfx.SpecError):
            scene.bind_hydrogen_bonds(identity, [(0, 1, 2)], candidate_distance=2.0)
        self.assertEqual(scene.spec.to_json(), before)
        scene.bind_hydrogen_bonds(identity, [(0, 1, 2)], candidate_distance=3.5)
        self.assertEqual(json.loads(scene.unresolved_overlays()), [])
        descriptor = json.loads(scene.spec.to_json())["interactions"][str(identity.value)]
        self.assertEqual(
            descriptor["geometry"],
            {
                "maximum_endpoint_distance": 3.5,
                "minimum_angle_degrees": 150.0,
                "maximum_pivot_end_distance": 2.5,
            },
        )

    def test_scientific_pose_metadata_distinguishes_snapshot_and_interval_candidates(self) -> None:
        """Worker results keep their original input and immutable frame column owners."""
        for candidates in (False, True):
            scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
            scene.add(
                molgfx.trajectory.bind(
                    structure=scene.structure_id,
                    source=molgfx.data.source("retained-hbond-frames"),
                    frame_count=2,
                )
            )
            scene.bind_trajectory(
                source_hash="retained-hbond-frames",
                start=molgfx.trajectory.frame(0, 0.0, ((0, 0, 0), (1, 0, 0), (2.8, 0, 0))),
                end=molgfx.trajectory.frame(1, 1.0, ((0, 4, 0), (1, 4, 0), (2.8, 4, 0))),
                sample_time=0.25,
            )
            identity = scene.add(
                molgfx.interaction.hydrogen_bonds(
                    structure=scene.structure_id,
                    source=molgfx.data.source("retained-hbond-result"),
                    geometry=(3.5, 150.0, 2.5),
                )
            )
            original = scene.molecular_pose(structure=scene.structure_id)
            scene.set_trajectory_time(structure=scene.structure_id, seconds=0.75)
            if not candidates:
                before = scene.spec.to_json(), scene.revision
                with self.assertRaises(molgfx.MolgfxError):
                    scene.bind_hydrogen_bonds(identity, [(0, 1, 2)], pose=original)
                self.assertEqual((scene.spec.to_json(), scene.revision), before)
                scene.set_trajectory_time(structure=scene.structure_id, seconds=0.25)
            scene.bind_hydrogen_bonds(
                identity,
                [(0, 1, 2)],
                pose=original,
                candidate_distance=3.5 if candidates else None,
            )
            _, column, _, metadata = scene.browser_bindings()[1][0]
            start, end, seconds = json.loads(metadata)
            self.assertEqual(seconds, 0.25)
            frames, intervals = scene.browser_motion_bindings()
            self.assertEqual(len(frames), 2)
            self.assertEqual((start, end), intervals[0][1:3])
            self.assertEqual(intervals[0][-1], 0.75 if candidates else 0.25)
            scene.set_trajectory_time(structure=scene.structure_id, seconds=0.9)
            self.assertIs(scene.browser_bindings()[1][0][1], column)
            self.assertEqual(scene.browser_bindings()[1][0][3], metadata)

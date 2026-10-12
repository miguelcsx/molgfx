"""Scientific assets cross the canonical widget transport with exact binary columns."""

import json
import struct
import unittest

import molframe
from test_hydrogen_bonds import PDB

import molgfx


class ScientificTransportTests(unittest.TestCase):
    """Use the real binding, Session and widget notifications."""

    def viewer(self, scene: molgfx.Scene) -> molgfx.Viewer:
        """Close each transport even when an assertion fails."""
        viewer = molgfx.Viewer(scene)
        self.addCleanup(viewer.close)
        return viewer

    def test_direct_scene_edits_and_transaction_publication_allow_viewer_callbacks(self) -> None:
        """Callbacks can inspect assets and revision after every authoring boundary."""
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        viewer = self.viewer(scene)
        identity = scene.add(molgfx.rep.spacefill(target="all"))
        scene.set_camera(scene.frame("all"))
        scene.set_opacity(identity, 0.5)
        with scene.transaction():
            scene.set_opacity(identity, 0.75)
            scene.set_visible(identity, visible=False)
        scene.apply(
            molgfx.ScenePatch(
                json.dumps(
                    {
                        "base_revision": scene.revision,
                        "operations": [
                            {"op": "set_visibility", "id": identity.value, "visible": True}
                        ],
                    }
                )
            )
        )
        self.assertEqual(viewer.patch_sequence, 5)
        self.assertEqual(viewer.revision, scene.revision)
        self.assertEqual(viewer.error, "")
        self.assertEqual(scene.browser_bindings(), ([], [], []))

    def test_binding_a_standalone_volume_resyncs_the_open_viewer(self) -> None:
        """A runtime binding publishes after the mutable scene borrow is released."""
        scene = molgfx.Scene()
        identity = scene.add(
            molgfx.density.volume(
                source=molgfx.data.source("widget-map"), dimensions=(2, 2, 2)
            ).isosurface(0.5)
        )
        viewer = self.viewer(scene)
        before = viewer.scene_spec
        scene.bind_volume(identity, [0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0])
        self.assertNotEqual(viewer.scene_spec, before)
        self.assertEqual(viewer.revision, scene.revision)
        self.assertEqual(viewer.structure_ids, [])
        self.assertEqual(viewer.patch_sequence, 0)
        self.assertEqual(viewer.volume_bindings[0][0], str(identity.value))
        self.assertEqual(
            struct.unpack("<8f", viewer.volume_bindings[0][1]),
            (0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        )
        first = scene.browser_bindings()[0][0][1]
        self.assertIs(scene.browser_bindings()[0][0][1], first)
        self.assertIs(viewer.volume_bindings[0][1], first)

    def test_hydrogen_bond_candidates_keep_their_envelope_and_cached_bytes(self) -> None:
        """Geometric policy survives transport and an appearance patch does not resend data."""
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        identity = scene.add(
            molgfx.interaction.hydrogen_bonds(
                structure=scene.structure_id,
                source=molgfx.data.source("widget-hbonds"),
                geometry=(3.5, 150.0, 2.5),
            )
        )
        viewer = self.viewer(scene)
        before = scene.to_json()
        with self.assertRaises(molgfx.SpecError):
            scene.bind_hydrogen_bonds(identity, [(0, 1, 3)], candidate_distance=3.5)
        self.assertEqual(scene.to_json(), before)
        self.assertEqual(viewer.hydrogen_bond_bindings, [])
        scene.bind_hydrogen_bonds(identity, [(0, 1, 2)], candidate_distance=3.5)
        _, column, distance, metadata = viewer.hydrogen_bond_bindings[0]
        self.assertEqual(metadata, "null")
        self.assertEqual(struct.unpack("<3I", column), (0, 1, 2))
        self.assertEqual(distance, 3.5)
        viewer.select("element O")
        self.assertEqual(viewer.patch_sequence, 1)
        self.assertIs(scene.browser_bindings()[1][0][1], column)
        self.assertIs(viewer.hydrogen_bond_bindings[0][1], column)
        viewer.sync_request += 1
        self.assertIs(viewer.hydrogen_bond_bindings[0][1], column)

    def test_surface_geometry_restores_complete_columns_and_scientific_metadata(self) -> None:
        """Triangles, normals and original atom ownership are transported independently."""
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        identity = scene.add(
            molgfx.rep.surf(target="all", source=molgfx.data.source("widget-surf"))
        )
        viewer = self.viewer(scene)
        scene.bind_molecular_surface(
            identity,
            json.dumps(
                {
                    "method": "surf",
                    "controls": {"probe_radius": 1.4, "density": 3.0, "all_components": True},
                    "mesh": {
                        "vertices": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
                        "normals": [[0, 0, 1]] * 3,
                        "faces": [[0, 1, 2]],
                        "atom_rows": [0, 1, 2],
                        "discarded_degenerate_faces": 2,
                    },
                }
            ),
        )
        bound_id, metadata, vertices, normals, faces, atoms = viewer.molecular_surface_bindings[0]
        self.assertEqual(bound_id, str(identity.value))
        method, controls, discarded, pose = json.loads(metadata)
        self.assertIsNone(pose)
        self.assertEqual((method, discarded), ("surf", 2))
        self.assertAlmostEqual(controls["probe_radius"], 1.4)
        self.assertEqual(struct.unpack("<9f", vertices), (0, 0, 0, 1, 0, 0, 0, 1, 0))
        self.assertEqual(struct.unpack("<9f", normals), (0, 0, 1) * 3)
        self.assertEqual(struct.unpack("<3I", faces), (0, 1, 2))
        self.assertEqual(struct.unpack("<3I", atoms), (0, 1, 2))
        fresh = scene.browser_bindings()[2][0]
        for cached, current in zip((vertices, normals, faces, atoms), fresh[2:], strict=True):
            self.assertIs(cached, current)
        self.assertEqual(viewer.revision, scene.revision)

    def test_runtime_bindings_are_refused_before_mutating_a_pending_transaction(self) -> None:
        """Transaction rollback cannot leak a scientific binding or its revision."""
        scene = molgfx.Scene()
        identity = scene.add(
            molgfx.density.volume(
                source=molgfx.data.source("transaction-grid"), dimensions=(2, 2, 2)
            ).isosurface(0.5)
        )
        before = scene.to_json()
        with scene.transaction(), self.assertRaises(ValueError):
            scene.bind_volume(identity, [0.0] * 8)
        self.assertEqual(scene.to_json(), before)
        self.assertEqual(scene.browser_bindings(), ([], [], []))

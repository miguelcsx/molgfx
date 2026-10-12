"""Analytical surface providers retain scientific identity across Python scene binding."""

import json
import os
import tempfile
import unittest
from pathlib import Path

import molframe
import numpy as np

import molgfx

PDB = b"HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n"


class MolecularSurfaceTests(unittest.TestCase):
    """Pending geometry resolves only with a matching scientific result."""

    def test_invalid_transport_does_not_consume_the_pending_surface(self) -> None:
        """Invalid atom owners and mismatched methods permit a valid retry."""
        scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
        identity = scene.add(molgfx.rep.surf(target="all", source=molgfx.data.source("surf")))
        self.assertEqual(
            [item.value for item in scene.unresolved_representations()], [identity.value]
        )
        mesh: dict[str, object] = {
            "vertices": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
            "normals": [[0, 0, 1]] * 3,
            "faces": [[0, 1, 2]],
            "atom_rows": [1, 1, 1],
        }
        data: dict[str, object] = {
            "method": "surf",
            "controls": {"probe_radius": 1.4, "density": 3.0, "all_components": True},
            "mesh": mesh,
        }
        before = scene.spec.to_json()
        with self.assertRaises(molgfx.MolgfxError):
            scene.bind_molecular_surface(identity, json.dumps(data))
        self.assertEqual(scene.spec.to_json(), before)
        mesh["atom_rows"] = [0, 0, 0]
        data["method"] = "msms"
        with self.assertRaises(molgfx.SpecError):
            scene.bind_molecular_surface(identity, json.dumps(data))
        self.assertEqual(scene.spec.to_json(), before)
        data["method"] = "surf"
        scene.bind_molecular_surface(identity, json.dumps(data))
        self.assertEqual(scene.unresolved_representations(), [])

    @unittest.skipUnless(os.environ.get("MOLFRAME_MSMS"), "requires caller-installed MSMS")
    def test_native_msms_output_binds_without_relabeling_its_provider_controls(self) -> None:
        """Authentic scientific output crosses the extension boundary as numerical data."""
        executable = os.environ["MOLFRAME_MSMS"]
        provider = molframe.surface.NativeSurfaceProvider(executable, "msms", probe=1.4)
        with tempfile.TemporaryDirectory() as directory:
            context = molframe.ExecutionContext(temp_directory=directory, temp_bytes=1_000_000_000)
            output = molframe.surface.native_surface(
                np.zeros((1, 3), dtype=np.float32),
                np.array([1.52], dtype=np.float32),
                np.array([0], dtype=np.uint32),
                provider,
                context=context,
            )
            self.assertEqual(output.method, "msms")
            scene = molgfx.Scene(molframe.read(PDB, format="pdb"))
            identity = scene.add(
                molgfx.rep.msms(target="all", source=molgfx.data.source("actual-msms"))
            )
            scene.bind_molecular_surface(identity, output)
            self.assertEqual(scene.unresolved_representations(), [])
            self.assertEqual(list(Path(directory).iterdir()), [])

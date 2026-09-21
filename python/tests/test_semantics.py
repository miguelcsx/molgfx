import unittest

import molframe
import molgfx


MMCIF = b"""data_one
_entry.id one
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
ATOM 1 N N ALA A 1 1 11.104 13.207 9.274 1.00 20.00
ATOM 2 C CA ALA A 1 1 12.560 13.318 9.111 1.00 20.00
"""


def structure():
    report = molframe.read_bytes(
        MMCIF,
        molframe.ReadOptions.standard(),
        name="one.cif",
    )
    return report.structure


class SceneSemanticsTests(unittest.TestCase):
    def test_transaction_commits_one_revision(self):
        scene = molgfx.Scene(structure())
        opacity = molgfx.visual.parameter(0.5, name="opacity_scale")
        style = molgfx.visual.style(
            color=molgfx.visual.color((30, 120, 220)),
            opacity=opacity,
        )
        representation = scene.add(
            molgfx.rep.spacefill(target=molgfx.sel.all()).visual(style)
        )
        revision = scene.revision

        with scene.transaction():
            scene.set_parameter(representation, opacity, 0.8)
            scene.set_opacity(representation, 0.9)

        self.assertEqual(scene.revision, revision + 1)
        self.assertIn("opacity_scale", scene.to_json())

    def test_invalid_transaction_rolls_back_every_operation(self):
        scene = molgfx.Scene(structure())
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
        revision = scene.revision
        before = scene.to_json()

        with self.assertRaises(molgfx.SpecError):
            with scene.transaction():
                scene.set_opacity(representation, 0.5)
                scene.set_opacity(2**63, 0.3)

        self.assertEqual(scene.revision, revision)
        self.assertEqual(scene.to_json(), before)

    def test_parameter_types_preserve_their_values(self):
        vector = molgfx.visual.vector_parameter(
            (1.0, 0.0, 0.0),
            name="direction",
        )
        color = molgfx.visual.color_parameter((10, 20, 30), name="tint")

        self.assertEqual(vector.default, (1.0, 0.0, 0.0))
        self.assertEqual(color.default, (10, 20, 30))


if __name__ == "__main__":
    unittest.main()

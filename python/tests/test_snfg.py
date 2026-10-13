"""The SNFG glyph table the renderer draws glycans with."""

import math
import unittest

import molframe

import molgfx
from molgfx import _engine

HEADER = (
    "data_snfg\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n"
    "_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n"
    "_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n"
    "_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n"
    "_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n"
    "_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n"
)
RING = ("C1", "C2", "C3", "C4", "C5", "O5")


def glycan(components: list[str]) -> molframe.Structure:
    """One unlinked six-atom ring per component, laid out along x."""
    lines = [HEADER]
    atom = 1
    for index, component in enumerate(components):
        for corner, name in enumerate(RING):
            angle = corner * math.tau / 6
            element = "O" if name.startswith("O") else "C"
            x = index * 4.0 + math.cos(angle) * 1.4
            y = math.sin(angle) * 1.4
            lines.append(
                f"HETATM {atom} {element} {name} . {component} A 1 {index + 1} "
                f"{x:.3f} {y:.3f} 0.0 1.00 10.0 {index + 1} A 1\n"
            )
            atom += 1
    lines.append("#\n")
    return molframe.read("".join(lines).encode(), name="glycan.cif")


class SnfgGlyphTests(unittest.TestCase):
    """The SNFG glyph table."""

    def test_spec_shapes_and_colours(self) -> None:
        expected = {
            "GLC": ("sphere", "#0090bc"),
            "MAN": ("sphere", "#00a651"),
            "GAL": ("sphere", "#ffd400"),
            "NAG": ("cube", "#0090bc"),
            "A2G": ("cube", "#ffd400"),
            "FUC": ("triangle", "#ed1c24"),
            "SIA": ("diamond", "#a54399"),
        }
        for component, (shape, color) in expected.items():
            found = _engine.snfg.glyph(component)
            assert found is not None, component
            self.assertEqual(found[:2], (shape, color), component)

    def test_lowercase_and_unknown(self) -> None:
        self.assertIsNotNone(_engine.snfg.glyph("nag"))
        self.assertIsNone(_engine.snfg.glyph("ALA"))


class SnfgRepresentationTests(unittest.TestCase):
    """The SNFG representation builder."""

    def test_builder_carries_scale(self) -> None:
        form = molgfx.rep.snfg(scale=1.5)
        self.assertIn("Snfg { scale: 1.5 }", form.explain())

    def test_scene_add_renders_one_symbol_per_monosaccharide(self) -> None:
        scene = molgfx.Scene(glycan(["NAG", "NAG", "MAN", "FUC"]))
        scene.add(molgfx.rep.snfg())
        scene.set_camera(scene.frame("all"))
        self.assertIn("representations: 1 (1 visible)", scene.explain())
        image = molgfx.Renderer().render_image(scene, size=(64, 64))
        self.assertTrue(any(image.pixels()))

    def test_invalid_scale_is_rejected(self) -> None:
        scene = molgfx.Scene(glycan(["NAG"]))
        with self.assertRaises(molgfx.SpecError):
            scene.add(molgfx.rep.snfg(scale=0.0))


if __name__ == "__main__":
    unittest.main()

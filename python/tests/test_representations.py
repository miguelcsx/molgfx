"""Every typed representation exposes exactly the controls its form defines.

`mypy.stubtest` cannot check these: a PyO3 function presents itself to Python
as an opaque callable, so a stub promising a keyword the binding never accepted
passes the stub check and fails only when somebody calls it. These tests call
each one.
"""

import unittest

import molgfx


class RepresentationControlTests(unittest.TestCase):
    def test_each_form_accepts_its_own_controls(self):
        cases = [
            (molgfx.rep.cartoon, {"width": 2.0, "style": "rocket"}),
            (molgfx.rep.ball_and_stick, {"radius": 0.3, "bond_radius": 0.2}),
            (molgfx.rep.spacefill, {"radius": 1.2}),
            (molgfx.rep.licorice, {"radius": 0.9, "bond_radius": 0.2}),
            (molgfx.rep.lines, {"width": 2.0}),
            (molgfx.rep.points, {"size": 4.0}),
            (
                molgfx.rep.surface,
                {
                    "kind": "gaussian",
                    "style": "mesh",
                    "probe_radius": 1.6,
                    "isolevel": 0.5,
                },
            ),
            (molgfx.rep.nucleic_acid, {"width": 1.4}),
            (molgfx.rep.bases, {"radius": 0.3}),
            (molgfx.rep.base_pairs, {"radius": 0.3, "bond_radius": 0.2}),
            (molgfx.rep.glycan, {"width": 1.5}),
        ]
        for constructor, controls in cases:
            with self.subTest(constructor.__name__):
                representation = constructor(target="all", **controls)
                self.assertIsInstance(representation, molgfx.Representation)

    def test_a_form_rejects_a_control_belonging_to_another_form(self):
        with self.assertRaises(TypeError):
            molgfx.rep.spacefill(target="all", bond_radius=0.2)
        with self.assertRaises(TypeError):
            molgfx.rep.lines(target="all", radius=0.2)
        with self.assertRaises(TypeError):
            molgfx.rep.cartoon(target="all", isolevel=0.5)

    def test_an_unknown_enumerated_control_names_the_valid_spellings(self):
        with self.assertRaises(ValueError) as caught:
            molgfx.rep.cartoon(target="all", style="squiggle")
        self.assertIn("squiggle", str(caught.exception))
        self.assertIn("ribbon", str(caught.exception))

        with self.assertRaises(ValueError) as caught:
            molgfx.rep.surface(target="all", kind="nope")
        self.assertIn("solvent_excluded", str(caught.exception))


if __name__ == "__main__":
    unittest.main()

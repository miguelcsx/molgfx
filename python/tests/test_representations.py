"""Every typed representation exposes exactly the controls its form defines.

`mypy.stubtest` cannot check these: a PyO3 function presents itself to Python
as an opaque callable, so a stub promising a keyword the binding never accepted
passes the stub check and fails only when somebody calls it. These tests call
each one.
"""

import unittest
from collections.abc import Callable
from typing import Any

import molgfx


def _with(constructor: Callable[..., Any], **controls: object) -> object:
    """Build a form with controls a stub cannot be handed as keywords.

    The stub is precise about which keyword each form takes, so a control
    belonging to another form cannot be written where mypy can see it. These
    tests exist precisely to watch the binding refuse it.
    """
    return constructor(target="all", **controls)


class RepresentationControlTests(unittest.TestCase):
    """Which controls each form accepts, and which it refuses."""

    def test_each_form_accepts_its_own_controls(self) -> None:
        cases: list[tuple[Callable[..., Any], dict[str, Any]]] = [
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

    def test_gap_controls_are_shared_by_all_polymer_forms(self) -> None:
        for constructor in (
            molgfx.rep.cartoon,
            molgfx.rep.backbone,
            molgfx.rep.trace,
            molgfx.rep.tube,
            molgfx.rep.putty,
        ):
            with self.subTest(constructor.__name__):
                self.assertIsInstance(
                    constructor(target="all", gaps="dashed"), molgfx.Representation
                )
                with self.assertRaisesRegex(ValueError, "hidden.*dashed"):
                    _with(constructor, gaps="unknown")
        with self.assertRaises(TypeError):
            _with(molgfx.rep.spacefill, gaps="dashed")

    def test_direction_wedges_require_a_boolean(self) -> None:
        """The binding refuses truthy strings and integers before scene mutation."""
        invalid_values: tuple[object, ...] = ("true", 1, 0, [])
        for invalid in invalid_values:
            with self.subTest(value=invalid), self.assertRaises(TypeError):
                _with(molgfx.rep.cartoon, direction_wedges=invalid)
        for enabled in (True, False):
            self.assertIsInstance(
                molgfx.rep.cartoon(target="all", direction_wedges=enabled),
                molgfx.Representation,
            )

    def test_a_form_rejects_a_control_belonging_to_another_form(self) -> None:
        with self.assertRaises(TypeError):
            _with(molgfx.rep.spacefill, bond_radius=0.2)
        with self.assertRaises(TypeError):
            _with(molgfx.rep.lines, radius=0.2)
        with self.assertRaises(TypeError):
            _with(molgfx.rep.cartoon, isolevel=0.5)

    def test_an_unknown_enumerated_control_names_the_valid_spellings(self) -> None:
        with self.assertRaises(ValueError) as caught:
            _with(molgfx.rep.cartoon, style="squiggle")
        self.assertIn("squiggle", str(caught.exception))
        self.assertIn("ribbon", str(caught.exception))

        with self.assertRaises(ValueError) as caught:
            _with(molgfx.rep.surface, kind="nope")
        self.assertIn("solvent_excluded", str(caught.exception))


if __name__ == "__main__":
    unittest.main()

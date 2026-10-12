"""Typed scalar-grid authoring keeps scientific controls and rejects invalid requests."""

import json
import unittest
from typing import Any

import molgfx


def _volume() -> molgfx.Volume:
    return molgfx.density.volume(source=molgfx.data.source("scalar-field"), dimensions=(5, 5, 5))


class VolumePresentations(unittest.TestCase):
    """Scalar-field adapters preserve the validated native presentation vocabulary."""

    def test_field_lines_preserve_every_authored_control(self) -> None:
        scene = molgfx.Scene()
        scene.add(
            _volume().field_lines(
                seed_stride=2,
                seed_gradient=(0.5, 2.0),
                step=0.125,
                length=(0.25, 4.0),
                critical_gradient=0.0001,
                max_steps=80,
                max_segments=12000,
                width_pixels=3.0,
                color=(0, 120, 255),
                opacity=0.75,
            )
        )
        descriptor = next(iter(json.loads(scene.to_json())["volumes"].values()))
        reading = descriptor["presentations"][0]
        self.assertEqual(reading["kind"], "field_lines")
        options = reading["options"]
        self.assertEqual(options["seed_stride"], 2)
        self.assertEqual(options["seed_gradient"], [0.5, 2.0])
        self.assertEqual(options["step"], 0.125)
        self.assertEqual(options["length"], [0.25, 4.0])
        self.assertAlmostEqual(options["critical_gradient"], 0.0001)
        self.assertEqual(options["max_steps"], 80)
        self.assertEqual(options["max_segments"], 12000)
        self.assertEqual(options["width_pixels"], 3.0)
        self.assertEqual(options["opacity"], 0.75)

    def test_orbital_and_isosurface_share_the_authored_boundary_style(self) -> None:
        scene = molgfx.Scene()
        scene.add(
            _volume()
            .orbital(0.25, positive=(255, 0, 0), negative=(0, 0, 255), style="mesh", width=0.2)
            .isosurface(0.5, style="mesh", width=0.2)
        )
        descriptor = next(iter(json.loads(scene.to_json())["volumes"].values()))
        orbital, boundary = descriptor["presentations"]
        self.assertEqual(orbital["kind"], "orbital")
        self.assertEqual(orbital["amplitude"], 0.25)
        self.assertNotEqual(orbital["positive"], orbital["negative"])
        self.assertEqual(orbital["style"], boundary["style"])

    def test_invalid_scientific_controls_do_not_insert_a_volume(self) -> None:
        scene = molgfx.Scene()
        before = scene.to_json()
        for volume in [_volume().field_lines(step=-1.0), _volume().orbital(0.0)]:
            with self.assertRaises(molgfx.SpecError):
                scene.add(volume)
            self.assertEqual(scene.to_json(), before)

    def test_unknown_field_line_controls_are_refused_by_the_binding(self) -> None:
        # The runtime refusal is the point; a typed literal would fail before the binding.
        invalid: Any = {"unrecognized_control": 1}
        with self.assertRaises(TypeError):
            _volume().field_lines(**invalid)


if __name__ == "__main__":
    unittest.main()

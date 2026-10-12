"""The SNFG glyph table the renderer draws glycans with."""

import unittest

from molgfx import _engine


class SnfgGlyphTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()

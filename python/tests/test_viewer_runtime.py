"""Tests for the viewer-owned browser runtime bridge."""

import json
from importlib.resources import files
import unittest

from molgfx.viewer._runtime import load_runtime


class ViewerRuntimeTests(unittest.TestCase):
    def test_runtime_bundle_is_cached_by_the_viewer_artifact(self):
        manifest = json.loads(
            files("molgfx.viewer")
            .joinpath("static", "runtime-manifest.json")
            .read_text(encoding="utf-8")
        )
        if not manifest.get("available", False):
            self.skipTest("the generated viewer runtime is not present")

        first = load_runtime()
        second = load_runtime()
        self.assertIs(first, second)
        self.assertEqual(first.key, manifest["content_key"])
        self.assertTrue(first.glue)
        self.assertTrue(first.wasm_gzip)

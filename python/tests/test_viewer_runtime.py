"""Packaged runtime integrity and deterministic notebook compression."""

import gzip
import json
import tempfile
import unittest
from hashlib import sha256
from importlib.resources import files
from pathlib import Path
from unittest.mock import patch

from molgfx.viewer._runtime import load_runtime


class ViewerRuntimeTests(unittest.TestCase):
    """Fail closed for missing, malformed, or checksum-mismatched assets."""

    def tearDown(self) -> None:
        """Do not retain temporary package data in the runtime cache."""
        load_runtime.cache_clear()

    def test_real_runtime_is_cached_and_preserves_exact_wasm_bytes(self) -> None:
        load_runtime.cache_clear()
        first = load_runtime()
        self.assertIs(first, load_runtime())
        wasm = files("molgfx").joinpath("static", "molgfx_wasm_bg.wasm").read_bytes()
        self.assertEqual(gzip.decompress(first.wasm_gzip), wasm)
        self.assertEqual(first.key, sha256(first.glue.encode() + b"\0" + wasm).hexdigest()[:16])
        load_runtime.cache_clear()
        self.assertEqual(load_runtime().wasm_gzip, first.wasm_gzip)

    def test_missing_runtime_fails_with_an_actionable_error(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            load_runtime.cache_clear()
            with (
                patch("molgfx.viewer._runtime.files", return_value=Path(directory)),
                self.assertRaisesRegex(RuntimeError, "build the web runtime"),
            ):
                load_runtime()

    def test_changed_runtime_bytes_fail_checksum_verification(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            static = Path(directory) / "static"
            static.mkdir()
            manifest = {
                "available": True,
                "glue": "molgfx_wasm.js",
                "wasm": "molgfx_wasm_bg.wasm",
                "content_key": "incorrect",
            }
            (static / "runtime-manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            (static / "molgfx_wasm.js").write_text("changed", encoding="utf-8")
            (static / "molgfx_wasm_bg.wasm").write_bytes(b"changed")
            (static / "anywidget.js").touch()
            (static / "viewer.css").touch()
            load_runtime.cache_clear()
            with (
                patch("molgfx.viewer._runtime.files", return_value=Path(directory)),
                self.assertRaisesRegex(RuntimeError, "checksum"),
            ):
                load_runtime()

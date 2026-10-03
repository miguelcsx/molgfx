"""Exercise prebuilt-runtime packaging failures before invoking the Rust build."""

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("build_backend", ROOT / "build_backend.py")
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Missing Python build backend")
BACKEND = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BACKEND)


class PackagingTests(unittest.TestCase):
    """No frontend tooling is needed, and invalid assets never reach maturin."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.static = self.root / "python/molgfx/static"
        self.static.mkdir(parents=True)
        (self.root / "pyproject.toml").write_text('[project]\nversion = "test"\n')
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "test"\n')
        for name in (
            "index.js",
            "viewer.js",
            "react.js",
            "anywidget.js",
            "viewer.css",
            "molgfx_wasm.js",
            "molgfx_wasm_bg.wasm",
        ):
            (self.static / name).write_bytes(name.encode())
        self.manifest = {
            "available": True,
            "version": "test",
            "glue": "molgfx_wasm.js",
            "wasm": "molgfx_wasm_bg.wasm",
            "content_key": hashlib.sha256(b"molgfx_wasm.js\0molgfx_wasm_bg.wasm").hexdigest()[:16],
            "sha256": {
                path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                for path in self.static.iterdir()
            },
        }
        self.write_manifest()

    def write_manifest(self) -> None:
        (self.static / "runtime-manifest.json").write_text(json.dumps(self.manifest))

    def test_prebuilt_runtime_is_sufficient(self) -> None:
        BACKEND.validate_runtime(self.root)

    def test_missing_manifest_is_rejected(self) -> None:
        (self.static / "runtime-manifest.json").unlink()
        with self.assertRaisesRegex(RuntimeError, "build --prefix web"):
            BACKEND.validate_runtime(self.root)

    def test_wrong_version_is_rejected(self) -> None:
        self.manifest["version"] = "stale"
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "Stale"):
            BACKEND.validate_runtime(self.root)

    def test_modified_bundle_is_rejected(self) -> None:
        (self.static / "anywidget.js").write_bytes(b"corrupt")
        with self.assertRaisesRegex(ValueError, "checksum"):
            BACKEND.validate_runtime(self.root)

    def test_missing_file_is_rejected(self) -> None:
        (self.static / "viewer.css").unlink()
        with self.assertRaisesRegex(ValueError, "inventory"):
            BACKEND.validate_runtime(self.root)

    def test_unmanifested_file_is_rejected(self) -> None:
        (self.static / "stale.js").touch()
        with self.assertRaisesRegex(ValueError, "inventory"):
            BACKEND.validate_runtime(self.root)

    def test_incomplete_manifest_is_rejected(self) -> None:
        self.manifest["sha256"].pop("anywidget.js")
        (self.static / "anywidget.js").unlink()
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "Incomplete"):
            BACKEND.validate_runtime(self.root)

    def test_wrong_content_key_is_rejected(self) -> None:
        self.manifest["content_key"] = "stale"
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "content key"):
            BACKEND.validate_runtime(self.root)

    def test_all_build_hooks_validate_before_delegating(self) -> None:
        for hook in ("build_wheel", "build_sdist", "build_editable"):
            with (
                self.subTest(hook=hook),
                patch.object(BACKEND, "validate_runtime", side_effect=ValueError("invalid")),
                patch.object(BACKEND.maturin, hook) as delegate,
                self.assertRaisesRegex(ValueError, "invalid"),
            ):
                getattr(BACKEND, hook)("output")
            delegate.assert_not_called()


if __name__ == "__main__":
    unittest.main()

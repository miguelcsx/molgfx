"""Gate Python source and wheel builds on the prebuilt canonical browser runtime."""

from __future__ import annotations

import hashlib
import json
import tomllib
from pathlib import Path
from typing import Any

import maturin

ROOT = Path(__file__).resolve().parent


def validate_runtime(root: Path = ROOT) -> None:
    """Reject incomplete, modified or wrong-version assets without invoking Node."""
    project = tomllib.loads((root / "pyproject.toml").read_text())["project"]
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    static = root / "python/molgfx/static"
    try:
        manifest = json.loads((static / "runtime-manifest.json").read_bytes())
    except (OSError, ValueError) as error:
        message = "Missing browser runtime; run npm run build --prefix web first"
        raise RuntimeError(message) from error
    if (
        not isinstance(manifest, dict)
        or manifest.get("available") is not True
        or manifest.get("version") != project["version"]
        or manifest["version"] != workspace["package"]["version"]
        or manifest.get("glue") != "molgfx_wasm.js"
        or manifest.get("wasm") != "molgfx_wasm_bg.wasm"
    ):
        message = "Stale or unavailable browser runtime"
        raise ValueError(message)
    hashes = manifest.get("sha256")
    required = {
        "index.js",
        "viewer.js",
        "react.js",
        "anywidget.js",
        "viewer.css",
        "molgfx_wasm.js",
        "molgfx_wasm_bg.wasm",
    }
    if not isinstance(hashes, dict) or not required.issubset(hashes):
        message = "Incomplete browser runtime manifest"
        raise ValueError(message)
    actual_files = {
        path.relative_to(static).as_posix() for path in static.rglob("*") if path.is_file()
    } - {"runtime-manifest.json"}
    if actual_files != set(hashes):
        message = "Browser runtime file inventory differs from its manifest"
        raise ValueError(message)
    for name, expected in hashes.items():
        path = Path(name)
        if path.is_absolute() or ".." in path.parts or "\\" in name:
            raise ValueError("Unsafe browser runtime path: " + name)
        if hashlib.sha256((static / path).read_bytes()).hexdigest() != expected:
            raise ValueError("Browser runtime checksum mismatch: " + name)
    key = hashlib.sha256(
        (static / manifest["glue"]).read_bytes() + b"\0" + (static / manifest["wasm"]).read_bytes()
    ).hexdigest()[:16]
    if key != manifest.get("content_key"):
        message = "Browser runtime content key mismatch"
        raise ValueError(message)


def build_wheel(
    wheel_directory: str,
    config_settings: dict[str, Any] | None = None,
    metadata_directory: str | None = None,
) -> str:
    """Build a wheel only after verifying its shipped runtime."""
    validate_runtime()
    return maturin.build_wheel(wheel_directory, config_settings, metadata_directory)


def build_sdist(sdist_directory: str, config_settings: dict[str, Any] | None = None) -> str:
    """Ship prebuilt runtime assets so source consumers need only Python and Rust."""
    validate_runtime()
    return maturin.build_sdist(sdist_directory, config_settings)


def build_editable(
    wheel_directory: str,
    config_settings: dict[str, Any] | None = None,
    metadata_directory: str | None = None,
) -> str:
    """Apply the same asset contract to editable Python installations."""
    validate_runtime()
    return maturin.build_editable(wheel_directory, config_settings, metadata_directory)


get_requires_for_build_wheel = maturin.get_requires_for_build_wheel
get_requires_for_build_sdist = maturin.get_requires_for_build_sdist
get_requires_for_build_editable = maturin.get_requires_for_build_editable
prepare_metadata_for_build_wheel = maturin.prepare_metadata_for_build_wheel
prepare_metadata_for_build_editable = maturin.prepare_metadata_for_build_editable

if __name__ == "__main__":
    validate_runtime()

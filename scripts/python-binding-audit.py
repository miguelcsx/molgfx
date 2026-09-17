#!/usr/bin/env python3
"""Audit the checked-in Python facade against one freshly built wheel."""

from __future__ import annotations

import argparse
import ast
import importlib
import sys
import tempfile
import zipfile
from pathlib import Path


LEGACY_SCENE_METHODS = {
    "add_particle",
    "primitive",
    "represent_isosurface",
    "represent_segmented_volume",
    "represent_volume",
    "set_representation_clipping",
    "set_representation_color",
    "set_representation_material",
}


def declarations(stub: Path) -> set[str]:
    tree = ast.parse(stub.read_text(encoding="utf-8"), filename=str(stub))
    return {
        node.name
        for node in tree.body
        if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef))
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--stub", type=Path, default=Path("python/molgfx/__init__.pyi"))
    arguments = parser.parse_args()
    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="molgfx-wheel-audit-") as directory:
        with zipfile.ZipFile(arguments.wheel) as archive:
            archive.extractall(directory)
        sys.path.insert(0, directory)
        package = importlib.import_module("molgfx")
        native = importlib.import_module("molgfx._native")
        exported = set(package.__all__)
        native_public = {name for name in dir(native) if not name.startswith("_")}
        missing = native_public - exported
        extra = exported - native_public
        if missing:
            failures.append(f"native names missing from __all__: {sorted(missing)}")
        if extra:
            failures.append(f"__all__ names absent from native module: {sorted(extra)}")
        undeclared = exported - declarations(arguments.stub)
        if undeclared:
            failures.append(f"exported names missing from stub: {sorted(undeclared)}")
        legacy = LEGACY_SCENE_METHODS.intersection(dir(package.Scene))
        if legacy:
            failures.append(f"legacy Scene methods remain public: {sorted(legacy)}")
        package.Representation.volume().volume_style(
            package.VolumeStyle.isosurface().sampling(1.0, 0.5)
        )
        package.SegmentationStyle(package.SegmentStyleTable.default())
    if failures:
        print("\n".join(f"ERROR: {failure}" for failure in failures))
        return 1
    print(f"Python wheel facade OK: {len(exported)} typed native exports")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

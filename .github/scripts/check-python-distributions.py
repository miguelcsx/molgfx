"""Prove that source archives build independently and preserve canonical wheel assets."""

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile
from pathlib import Path


def compare(source: Path, wheel: Path) -> None:
    """Compare all shipped runtime bytes and distribution versions."""
    with tarfile.open(source, "r:gz") as archive, zipfile.ZipFile(wheel) as binary:
        roots = {member.name.split("/")[0] for member in archive.getmembers()}
        if len(roots) != 1:
            raise ValueError("Source archive must contain one project root")
        root = roots.pop()

        def read(name: str) -> bytes:
            member = archive.extractfile(root + "/" + name)
            if member is None:
                raise ValueError("Source archive missing " + name)
            return member.read()

        project = tomllib.loads(read("pyproject.toml").decode())["project"]
        read("build_backend.py")
        manifest_bytes = read("python/molgfx/static/runtime-manifest.json")
        manifest = json.loads(manifest_bytes)
        if manifest["version"] != project["version"]:
            raise ValueError("Source archive has a stale browser runtime")
        metadata = next(name for name in binary.namelist() if name.endswith(".dist-info/METADATA"))
        if f"Version: {project['version']}\n" not in binary.read(metadata).decode():
            raise ValueError("Source and wheel versions differ")
        prefix = "molgfx/static/"
        expected_names = set(manifest["sha256"]) | {"runtime-manifest.json"}
        wheel_names = {
            name.removeprefix(prefix)
            for name in binary.namelist()
            if name.startswith(prefix) and not name.endswith("/")
        }
        source_prefix = root + "/python/" + prefix
        source_names = {
            member.name.removeprefix(source_prefix)
            for member in archive.getmembers()
            if member.name.startswith(source_prefix) and member.isfile()
        }
        if wheel_names != expected_names or source_names != expected_names:
            raise ValueError("Source or wheel runtime inventory differs from its manifest")
        for name in expected_names:
            content = read("python/" + prefix + name)
            if content != binary.read(prefix + name):
                raise ValueError("Source/wheel runtime mismatch: " + name)
            if name != "runtime-manifest.json":
                if hashlib.sha256(content).hexdigest() != manifest["sha256"][name]:
                    raise ValueError("Archive runtime checksum mismatch: " + name)
        if any(name.endswith("build_backend.py") for name in binary.namelist()):
            raise ValueError("The build backend must not leak into the wheel")
    print(f"Identical source/wheel assets: {source.name}, {wheel.name}")


def rebuild(source: Path, destination: Path) -> None:
    """Build an extracted archive with no sibling checkout and blocked frontend tools."""
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="molgfx-sdist-") as temporary:
        base = Path(temporary)
        with tarfile.open(source, "r:gz") as archive:
            archive.extractall(base, filter="data")
        projects = list(base.glob("*/pyproject.toml"))
        if len(projects) != 1 or (base / "molframe").exists():
            raise ValueError("Source archive does not have an independent project root")
        project = projects[0].parent
        if (project / "web/node_modules").exists():
            raise ValueError("Source archive must not carry frontend build dependencies")
        blocked = base / "blocked-tools"
        blocked.mkdir()
        for tool in ("node", "npm", "npx", "wasm-pack"):
            executable = blocked / tool
            executable.write_text(f"#!/bin/sh\necho 'Forbidden sdist tool: {tool}' >&2\nexit 97\n")
            executable.chmod(0o755)
        environment = dict(os.environ)
        environment["PATH"] = str(blocked) + os.pathsep + environment["PATH"]
        subprocess.run(
            [sys.executable, "-m", "build", "--wheel", "--outdir", str(destination.resolve())],
            cwd=project,
            env=environment,
            check=True,
        )
    wheels = list(destination.glob("*.whl"))
    if len(wheels) != 1:
        raise ValueError("Expected exactly one rebuilt wheel in " + str(destination))
    compare(source, wheels[0])
    print("Source archive rebuilt without Node, npm, wasm-pack or an adjacent MolFrame checkout")


def main() -> None:
    """Check supplied archives, optionally performing a real source rebuild."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("wheels", nargs="*", type=Path)
    parser.add_argument("--rebuild", type=Path, metavar="WHEEL_DIRECTORY")
    args = parser.parse_args()
    if not args.wheels and args.rebuild is None:
        parser.error("Provide wheels or --rebuild")
    for wheel in args.wheels:
        compare(args.source, wheel)
    if args.rebuild is not None:
        rebuild(args.source, args.rebuild)


if __name__ == "__main__":
    main()

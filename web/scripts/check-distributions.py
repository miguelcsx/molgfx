"""Compare the actual npm archive and wheel against the shared asset manifest."""

import hashlib
import json
import sys
import tarfile
import zipfile
from pathlib import Path


def check(npm_path: str, wheel_path: str) -> None:
    """Reject missing, stale or separately-built assets in either distribution."""
    with tarfile.open(npm_path, "r:gz") as npm, zipfile.ZipFile(wheel_path) as wheel:
        member = npm.extractfile("package/dist/runtime-manifest.json")
        if member is None:
            raise ValueError("npm archive is missing its asset manifest")
        manifest_bytes = member.read()
        manifest = json.loads(manifest_bytes)
        if wheel.read("molgfx/static/runtime-manifest.json") != manifest_bytes:
            raise ValueError("npm and wheel asset manifests differ")
        package_member = npm.extractfile("package/package.json")
        if package_member is None:
            raise ValueError("npm archive is missing package.json")
        package = json.load(package_member)
        metadata_name = next(
            name for name in wheel.namelist() if name.endswith(".dist-info/METADATA")
        )
        metadata = wheel.read(metadata_name).decode()
        if (
            package["name"] != "molgfx"
            or package["version"] != manifest["version"]
            or f"Version: {manifest['version']}\n" not in metadata
        ):
            raise ValueError("npm, wheel and manifest versions differ")
        for name, expected in manifest["sha256"].items():
            member = npm.extractfile("package/dist/" + name)
            if member is None:
                raise ValueError("npm archive is missing " + name)
            npm_bytes = member.read()
            wheel_bytes = wheel.read("molgfx/static/" + name)
            if npm_bytes != wheel_bytes or hashlib.sha256(npm_bytes).hexdigest() != expected:
                raise ValueError("Distribution asset mismatch: " + name)
        wasm_hash = manifest["sha256"][manifest["wasm"]]
        print(f"Identical npm/wheel assets; wasm SHA256 {wasm_hash}")


if __name__ == "__main__":
    npm_path, *wheel_paths = sys.argv[1:]
    for argument in wheel_paths:
        paths = sorted(Path().glob(argument)) if "*" in argument else [Path(argument)]
        if not paths:
            raise ValueError("No wheel matched " + argument)
        for wheel_path in paths:
            check(npm_path, str(wheel_path))

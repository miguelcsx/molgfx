"""Load the immutable browser runtime published by molgfx-viewer.

The TypeScript viewer owns runtime initialization. This module only bridges its
generated package artifact into AnyWidget's inline-runtime channel, which is
needed because notebook frontends evaluate widget modules from blob URLs.
"""

from dataclasses import dataclass
from functools import lru_cache
import gzip
from hashlib import sha256
import json
from importlib.resources import files


@dataclass(frozen=True, slots=True)
class RuntimeBundle:
    """The viewer artifact in the form required by the AnyWidget adapter."""

    glue: str = ""
    wasm_gzip: bytes = b""
    key: str = ""


@lru_cache(maxsize=1)
def load_runtime() -> RuntimeBundle:
    """Load and validate the generated molgfx-viewer runtime artifact."""
    static = files("molgfx.viewer").joinpath("static")
    try:
        manifest = json.loads(
            static.joinpath("runtime-manifest.json").read_text(encoding="utf-8")
        )
    except (FileNotFoundError, json.JSONDecodeError):
        return RuntimeBundle()

    if not manifest.get("available", False):
        return RuntimeBundle()

    glue_name = manifest["glue"]
    wasm_name = manifest["wasm"]
    glue = static.joinpath(glue_name).read_text(encoding="utf-8")
    wasm = static.joinpath(wasm_name).read_bytes()
    content_key = sha256(glue.encode() + b"\0" + wasm).hexdigest()[:16]
    if content_key != manifest["content_key"]:
        raise RuntimeError("the packaged MolGFX viewer runtime failed its checksum")

    return RuntimeBundle(
        glue=glue,
        wasm_gzip=gzip.compress(wasm, mtime=0),
        key=content_key,
    )

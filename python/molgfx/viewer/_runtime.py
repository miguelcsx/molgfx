"""Validate and inline the shared browser runtime for notebook blob modules."""

import gzip
import json
from dataclasses import dataclass
from functools import lru_cache
from hashlib import sha256
from importlib.resources import files
from typing import cast


@dataclass(frozen=True, slots=True)
class RuntimeBundle:
    """Immutable glue, compressed WASM and their content identity."""

    glue: str
    wasm_gzip: bytes
    key: str


@lru_cache(maxsize=1)
def load_runtime() -> RuntimeBundle:
    """Load packaged assets, failing explicitly when missing or inconsistent."""
    static = files("molgfx").joinpath("static")
    message = "MolGFX browser assets are missing or invalid; build the web runtime first"
    try:
        value: object = json.loads(
            static.joinpath("runtime-manifest.json").read_text(encoding="utf-8")
        )
    except (OSError, ValueError) as error:
        raise RuntimeError(message) from error
    if not isinstance(value, dict):
        raise TypeError(message)
    manifest = cast("dict[str, object]", value)
    if manifest.get("available") is not True:
        raise RuntimeError(message)
    if manifest.get("glue") != "molgfx_wasm.js" or manifest.get("wasm") != "molgfx_wasm_bg.wasm":
        raise RuntimeError(message)
    try:
        glue = static.joinpath("molgfx_wasm.js").read_text(encoding="utf-8")
        wasm = static.joinpath("molgfx_wasm_bg.wasm").read_bytes()
        for asset in ("anywidget.js", "viewer.css"):
            if not static.joinpath(asset).is_file():
                raise FileNotFoundError
    except OSError as error:
        raise RuntimeError(message) from error
    content_key = sha256(glue.encode() + b"\0" + wasm).hexdigest()[:16]
    if content_key != manifest.get("content_key"):
        message = "the packaged MolGFX viewer runtime failed its checksum"
        raise RuntimeError(message)
    return RuntimeBundle(glue=glue, wasm_gzip=gzip.compress(wasm, mtime=0), key=content_key)

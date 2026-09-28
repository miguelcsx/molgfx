"""Write the manifest for a freshly built Python viewer runtime."""

from hashlib import sha256
import json
from pathlib import Path


STATIC = Path("python/molgfx/viewer/static")
GLUE = STATIC / "molgfx_wasm.js"
WASM = STATIC / "molgfx_wasm_bg.wasm"

content_key = sha256(GLUE.read_bytes() + b"\0" + WASM.read_bytes()).hexdigest()[:16]
manifest = {
    "available": True,
    "glue": GLUE.name,
    "wasm": WASM.name,
    "content_key": content_key,
}
(STATIC / "runtime-manifest.json").write_text(
    json.dumps(manifest, separators=(",", ":")) + "\n", encoding="utf-8"
)

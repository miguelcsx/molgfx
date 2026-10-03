"""Emit actual widget runtime and transport states for browser integration tests."""

import base64
import json

from test_commands import _structure

from molgfx import Viewer


def _state(viewer: Viewer) -> dict[str, object]:
    result: dict[str, object] = viewer.get_state()
    result.pop("layout", None)
    result["_runtime_wasm"] = base64.b64encode(viewer._runtime_wasm).decode()
    result["structure_payloads"] = [
        base64.b64encode(payload).decode() for payload in viewer.structure_payloads
    ]
    return result


def main() -> None:
    """Build all fixture states through the actual extension and shared assets."""
    viewer = Viewer(_structure())
    try:
        initial = _state(viewer)
        viewer.select("resname HEM")
        patched = _state(viewer)
        viewer.scene.add_structure(_structure())
        replacement = _state(viewer)
        viewer.sync_request += 1
        resynced = _state(viewer)
    finally:
        viewer.close()
    print(
        json.dumps(
            {
                "initial": initial,
                "patched": patched,
                "replacement": replacement,
                "resynced": resynced,
            }
        )
    )


if __name__ == "__main__":
    main()

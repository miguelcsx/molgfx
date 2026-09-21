from molgfx import Scene, ScenePatch

class Viewer:
    scene_spec: str
    scene_patch: str
    patch_sequence: int
    structure_ids: list[int]
    structure_names: list[str]
    structure_payloads: list[bytes]
    pick: dict[str, object]
    selection: str
    camera: dict[str, object]
    error: str
    def __init__(self, scene: Scene, **kwargs: object) -> None: ...
    def apply(self, patch: ScenePatch) -> None: ...
    def refresh(self) -> None: ...

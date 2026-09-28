from molgfx import CommandResult, Scene, ScenePatch, Session

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
    interaction_event: dict[str, object]
    sequence_intervals: dict[str, object]
    focus_preset: str
    measurement_request: dict[str, object]
    volume_sigma: dict[str, object]
    trajectory_frame: dict[str, object]
    trajectory_time: dict[str, object]
    revision: int
    sync_request: int
    def __init__(self, scene: Scene, **kwargs: object) -> None: ...
    def apply(self, patch: ScenePatch) -> None: ...

class Workbench(Viewer):
    workbench: bool
    history: list[str]
    command_request: dict[str, object]
    command_reply: dict[str, object]
    session: Session
    def __init__(self, source: Session | Scene | object, **kwargs: object) -> None: ...
    @property
    def scene(self) -> Scene: ...
    def execute(self, text: str) -> CommandResult: ...

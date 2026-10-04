"""Exercise snapshot image/pick restoration through the installed native binding."""

import hashlib
import json

import molframe

import molgfx

PDB = b"ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n"
scene = molgfx.Scene(molframe.read(PDB, name="snapshot.pdb"))
session = molgfx.Session(scene)
session.execute("show spacefill as atoms; color red, all")
camera = scene.frame(molgfx.sel.all(), aspect=1.0)
scene.set_camera(camera)
session.execute(
    'plane {"structure":1,"center":[11.104,6.134,-6.504],'
    '"normal":[0.0,0.0,1.0],"tangent":[1.0,0.0,0.0],"size":[6.0,6.0],'
    '"color":[226,232,240,255]}'
)
session.snapshot_save("initial")
projected = camera.project((11.104, 6.134, -6.504), (128, 128))
assert projected is not None
x, y, _ = projected


def capture() -> tuple[bytes, tuple[str, int | None, int | None] | None]:
    """Render a fresh complete exposure and inspect its molecular pick."""
    renderer = molgfx.Renderer(profile=molgfx.profile.converged())
    pixels = renderer.render_image(scene, size=(128, 128)).pixels()
    pick = renderer.pick(int(x), int(y))
    identity = None if pick is None else (pick.kind, pick.dataset, pick.row)
    return pixels, identity


initial_pixels, initial_pick = capture()
assert initial_pick is not None, "the captured atom must be pickable"
session.execute("hide @atoms; color blue, all")
changed_pixels, ignored_pick = capture()
assert initial_pixels != changed_pixels, "mutating the captured style must change pixels"
session.snapshot_restore("initial")
restored_pixels, restored_pick = capture()
assert restored_pixels == initial_pixels, "restoration must recover the captured image"
assert restored_pick == initial_pick, "restoration must recover semantic pick identity"
session.undo()
undo_pixels, ignored_pick = capture()
assert undo_pixels == changed_pixels, "undo must recover the scene before restore"
print(
    json.dumps(
        {
            "initial": hashlib.sha256(initial_pixels).hexdigest(),
            "changed": hashlib.sha256(changed_pixels).hexdigest(),
            "restored": hashlib.sha256(restored_pixels).hexdigest(),
            "undo": hashlib.sha256(undo_pixels).hexdigest(),
            "pick": initial_pick,
            "revision": scene.revision,
        }
    )
)

"""Repeated renders of one scene on one adapter must agree byte for byte.

Not a unittest: it needs a graphics API and runs against an installed wheel,
like `headless_render.py`. The contract is determinism on a single adapter;
bytes may differ between adapters, which is why nothing is compared across them.
"""

import json
import sys

import molframe
import molgfx

PDB = b"""\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 10.00           C
ATOM      3  C   ALA A   1       2.000   1.400   0.000  1.00 10.00           C
ATOM      4  O   ALA A   1       1.300   2.400   0.100  1.00 10.00           O
ATOM      5  N   GLY A   2       3.300   1.450   0.000  1.00 10.00           N
ATOM      6  CA  GLY A   2       4.000   2.700   0.100  1.00 10.00           C
ATOM      7  C   GLY A   2       5.500   2.500   0.000  1.00 10.00           C
HETATM    8  C1  LIG B   1       2.500   3.500   3.000  1.00 10.00           C
HETATM    9  O1  LIG B   1       3.500   4.000   3.200  1.00 10.00           O
END
"""


def render(build) -> bytes:
    scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
    build(scene)
    image = molgfx.Renderer().render_image(scene, size=(96, 72))
    quality = json.loads(image.quality_json())
    assert quality["complete"], quality
    return bytes(image.pixels())


def spacefill(scene):
    scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))


def pocket(scene):
    scene.pocket("resname LIG", style=molgfx.PocketStyle(near=5.0, mid=9.0))


failures = []
for name, build in (("spacefill", spacefill), ("pocket", pocket)):
    first, second = render(build), render(build)
    if first != second:
        differing = sum(a != b for a, b in zip(first, second, strict=True))
        failures.append(f"{name}: {differing} bytes differ between identical renders")
    elif not any(first):
        failures.append(f"{name}: the render is entirely transparent black")
    else:
        print(f"{name}: two renders are byte-identical")
def camera_path_frames():
    scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
    scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))

    def at(x):
        return molgfx.Camera(position=(x, 3.0, 14.0), target=(2.5, 2.0, 1.0))

    path = molgfx.CameraPath([(0.0, at(-4.0)), (1.0, at(9.0))])
    renderer = molgfx.Renderer()
    frames = renderer.render_camera_path(scene, path, size=(64, 48), fps=3)
    return [bytes(frame.pixels()) for frame in frames]


first, second = camera_path_frames(), camera_path_frames()
if len(first) != 4:
    failures.append(f"a one-second path at 3 fps should yield 4 frames, got {len(first)}")
elif first != second:
    failures.append("camera-path frames differ between identical runs")
elif len(set(first)) != 4:
    failures.append("camera-path frames are not all distinct: the camera did not move")
else:
    print("camera path: 4 distinct, repeatable frames")

if failures:
    sys.exit("\n".join(failures))

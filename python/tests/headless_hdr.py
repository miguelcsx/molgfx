"""Export a converged HDR fixture and require unclipped highlights.

Needs a graphics API and a caller-supplied structure file, not unittest discovery.
Run with 4HHB as the input, then inspect the saved file with OpenEXR's exrheader.
"""

import argparse
import json
import math
import struct
from pathlib import Path

import molframe

import molgfx

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("structure", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()

scene = molgfx.Scene(molframe.read(args.structure))
scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
size = (512, 384)
renderer = molgfx.Renderer(profile=molgfx.profile.converged())
empty_target_rejected = False
try:
    renderer.render_hdr_image(scene, size=(0, size[1]))
except molgfx.SpecError:
    empty_target_rejected = True
assert empty_target_rejected, "an empty HDR target must fail before rendering"

image = renderer.render_hdr_image(scene, size=size)
quality = json.loads(image.quality_json())
assert quality["complete"], quality
assert (image.width, image.height) == size

maximum = 0.0
for red, green, blue, alpha in struct.iter_unpack("<eeee", image.rgba16f()):
    assert all(math.isfinite(value) for value in (red, green, blue, alpha))
    assert alpha == 1.0, f"opaque HDR capture has non-coverage alpha: {alpha}"
    maximum = max(maximum, red, green, blue)
assert maximum > 1.0, f"HDR highlights were clipped: max RGB = {maximum}"

args.output.parent.mkdir(parents=True, exist_ok=True)
wrong_format = args.output.with_suffix(".png")
assert not wrong_format.exists(), "the refusal probe requires a new path"
wrong_format_rejected = False
try:
    image.save(wrong_format)
except ValueError:
    wrong_format_rejected = True
assert wrong_format_rejected, "HDR save must reject non-EXR formats"
assert not wrong_format.exists(), "a rejected save must not create a file"
image.save(args.output)
assert args.output.read_bytes() == image.exr_bytes()
print(json.dumps({"file": str(args.output), "max_rgb": maximum, "quality": quality}, indent=2))

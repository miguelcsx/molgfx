"""Render real PNG and OpenEXR images through an installed wheel.

Not a unittest (the name is not `test*.py`): it needs a graphics API, which the
workflow provides, and it must run against the wheel, not a development build.
"""

import json
import os
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import cast

import molframe

import molgfx

MMCIF = b"""data_one
_entry.id one
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
ATOM 1 N N ALA A 1 1 11.104 13.207 9.274 1.00 20.00
ATOM 2 C CA ALA A 1 1 12.560 13.318 9.111 1.00 20.00
"""

info = molgfx.system_info()
print(json.dumps(info, indent=2))
# `system_info` hands back parsed JSON, so each entry is whatever the engine
# reported; the two this script reads are pinned here rather than guessed.
backends = cast("list[str]", info["compiled_backends"])
adapters = cast("list[dict[str, str]]", info["available_adapters"])
if sys.platform.startswith("linux") and "gl" not in backends:
    sys.exit("the Linux wheel was built without the OpenGL backend")

expected_backend = os.environ.get("WGPU_BACKEND")
if expected_backend:
    discovered = {adapter["backend"] for adapter in adapters}
    assert discovered == {expected_backend}, (expected_backend, discovered)

scene = molgfx.Scene(molframe.read(MMCIF, name="one.cif"))
scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
image = molgfx.Renderer().render_image(scene, size=(64, 64))
assert (image.width, image.height) == (64, 64)
pixels = image.pixels()
assert any(pixel != 0 for pixel in pixels), "render output is entirely transparent black"
assert image.png_bytes().startswith(b"\x89PNG")
print("rendered a non-empty 64x64 PNG")

effect = molgfx.effect.backdrop_gradient(top=(11, 23, 37), bottom=(11, 23, 37), glow_strength=0)
assert effect.kind == "backdrop"
profile = molgfx.profile.interactive().with_effect(effect)
assert profile.effect(effect) is not None
custom = molgfx.Renderer(profile=profile).render_image(scene, size=(64, 64))
assert bytes(custom.pixels()) != bytes(image.pixels()), (
    "explicit backdrop did not affect the output"
)
restored = profile.without_effect(effect)
assert restored.effect(effect) is None
try:
    molgfx.effect.bloom(intensity=float("nan"))
except molgfx.SpecError:
    pass
else:
    message = "non-finite bloom intensity was accepted"
    raise AssertionError(message)
print("profile effects change image pixels and reject invalid settings")

for surface_color in (molgfx.color.element(), (230, 60, 65)):
    surface_images: list[bytes] = []
    for opacity in (1.0, 0.1):
        surface_scene = molgfx.Scene(molframe.read(MMCIF, name="surface.cif"))
        surface_scene.add(molgfx.rep.surface(target="all", color=surface_color, opacity=opacity))
        surface_images.append(
            molgfx.Renderer(profile=profile).render_image(surface_scene, size=(64, 64)).pixels()
        )
    empty_surface = molgfx.Scene(molframe.read(MMCIF, name="surface.cif"))
    background = (
        molgfx.Renderer(profile=profile).render_image(empty_surface, size=(64, 64)).pixels()
    )
    contrast = [
        sum(
            abs(pixel - base)
            for i, (pixel, base) in enumerate(zip(pixels, background, strict=True))
            if i % 4 != 3
        )
        for pixels in surface_images
    ]
    assert contrast[1] < contrast[0] * 0.65, "surface opacity did not reveal the backdrop"
print("surface opacity affects fixed and element-coloured presentation")

hdr_renderer = molgfx.Renderer(profile=molgfx.profile.converged())
for invalid_size in ((0, 64), (64, 0)):
    try:
        hdr_renderer.render_hdr_image(scene, size=invalid_size)
    except molgfx.SpecError:
        pass
    else:
        message = f"HDR render accepted an empty image: {invalid_size}"
        raise AssertionError(message)
hdr = hdr_renderer.render_hdr_image(scene, size=(64, 64))
assert isinstance(hdr, molgfx.HdrImage)
assert (hdr.width, hdr.height) == (64, 64)
assert len(hdr.rgba16f()) == 64 * 64 * 8
hdr_quality = json.loads(hdr.quality_json())
assert hdr_quality["complete"], hdr_quality
assert hdr_quality["samples_completed"] == hdr_quality["samples_required"]
assert not hdr_quality["progressive"]
assert hdr_quality["full_residency"]
exr = hdr.exr_bytes()
assert exr[:4] == b"\x76\x2f\x31\x01"
with TemporaryDirectory() as directory:
    exr_path = Path(directory) / "capture.exr"
    hdr.save(exr_path)
    assert exr_path.read_bytes() == exr
    hdr.save(str(exr_path))
    assert exr_path.read_bytes() == exr
    for suffix in (".png", ".jpg", ".exr.png", ""):
        rejected_path = Path(directory) / f"rejected{suffix}"
        try:
            hdr.save(rejected_path)
        except ValueError:
            pass
        else:
            message = f"HDR save accepted a non-EXR path: {rejected_path}"
            raise AssertionError(message)
        assert not rejected_path.exists(), "extension rejection must precede writing"
print("rendered and saved a complete scene-linear 64x64 OpenEXR")

# The camera primitives must agree with where the renderer draws: an atom's
# projected pixel lies on the drawn atom, and a far corner does not.
view = scene.frame(molgfx.sel.all(), aspect=1.0)
scene.set_camera(view)
framed = molgfx.Renderer().render_image(scene, size=(128, 128))
framed_pixels = bytes(framed.pixels())


def differs_from_background(x: int, y: int) -> bool:
    """Whether a pixel is far from the gradient background at the corner."""
    at = (y * 128 + x) * 4
    return any(
        abs(framed_pixels[at + channel] - framed_pixels[channel]) > 24 for channel in range(3)
    )


projected = view.project((12.560, 13.318, 9.111), (128, 128))
assert projected is not None, "a point in front of the eye projects"
x, y, _ = projected
assert differs_from_background(int(x), int(y)), ("projected atom is not drawn", x, y)
assert not differs_from_background(2, 2), "the corner is background"
print("projection lands on the drawn atom")

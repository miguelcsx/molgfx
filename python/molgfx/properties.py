"""One-line scalar colouring from an array of per-atom values."""

from collections.abc import Iterable
from hashlib import sha256
from struct import pack

from ._engine import ColorSpec, ScalarProperty, Scene, StructureId, color

__all__ = ["bind", "by_array"]


def bind(
    scene: Scene,
    values: Iterable[float],
    *,
    name: str | None = None,
    domain: tuple[float, float] | None = None,
    structure: StructureId | None = None,
) -> ScalarProperty:
    """Bind one value per atom with a source hash derived from the data.

    This is `Scene.bind_property` without an invented `source_hash`: equal
    arrays hash equally, so the binding is reproducible. `name` defaults to
    `values-<hash prefix>`; property names are unique per scene, so pass your
    own when you want a readable one.
    """
    numbers = [float(value) for value in values]
    digest = sha256(pack(f"<{len(numbers)}d", *numbers)).hexdigest()
    return scene.bind_property(
        structure=structure or scene.structure_id,
        name=name or f"values-{digest[:12]}",
        source_hash=digest,
        values=numbers,
        domain=domain,
    )


def by_array(
    scene: Scene,
    values: Iterable[float],
    *,
    ramp: str = "viridis",
    domain: tuple[float, float] | None = None,
    name: str | None = None,
) -> ColorSpec:
    """Return a colour that maps one per-atom value through a ramp.

    `domain` defaults to the data's `(min, max)`. Pass the result as `color=`
    of a representation, for example
    `scene.add(molgfx.rep.spacefill(target="all", color=molgfx.by_array(scene, b)))`.
    The first structure is coloured; for another structure call `bind(..., structure=...)`
    and `molgfx.color.property`. The declarative pieces remain available: `Scene.bind_property` plus
    `molgfx.color.property`.
    """
    numbers = [float(value) for value in values]
    if not numbers:
        message = "values must not be empty"
        raise ValueError(message)
    span = domain if domain is not None else (min(numbers), max(numbers))
    prop = bind(scene, numbers, name=name, domain=span)
    return color.property(property=prop, ramp=ramp, domain=span)

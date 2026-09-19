"""Project-owned math identities.

Vectors, quaternions, matrices, bounds, colour and the camera.
"""

from typing import Sequence, final

__all__ = [
    "Aabb",
    "BoundingSphere",
    "Camera",
    "CurveSample",
    "Mat3",
    "Mat4",
    "Projection",
    "Quat",
    "Rgba8",
    "TransportFrame",
    "Vec2",
    "Vec3",
    "Vec4",
    "parallel_transport",
    "round_u8",
    "sample_catmull_rom",
    "sample_catmull_rom_demanding",
    "sample_catmull_rom_fixed",
    "sample_cubic_bezier",
    "truncate_u16",
    "unit_to_grid",
    "unorm8",
]

def parallel_transport(samples: Sequence[CurveSample]) -> list[TransportFrame]: ...
def round_u8(value: float) -> int: ...
def sample_catmull_rom(points: Sequence[Vec3], tolerance: float, max_steps: int) -> list[CurveSample]: ...
def sample_catmull_rom_demanding(points: Sequence[Vec3], tolerance: float, max_steps: int, demand: Sequence[float]) -> list[CurveSample]: ...
def sample_catmull_rom_fixed(points: Sequence[Vec3], steps_per_segment: int) -> list[CurveSample]: ...
def sample_cubic_bezier(control: Sequence[Vec3], segments: int) -> list[Vec3]: ...
def truncate_u16(value: float) -> int: ...
def unit_to_grid(value: float, levels: int) -> int: ...
def unorm8(value: float) -> int: ...

@final
class CurveSample:
    position: Vec3
    tangent: Vec3
    segment: int
    parameter: float

@final
class TransportFrame:
    tangent: Vec3
    normal: Vec3
    binormal: Vec3

@final
class Vec2:
    def __new__(cls, x: float, y: float) -> Vec2: ...
    x: float
    y: float
    ZERO: Vec2
    def to_tuple(self) -> tuple[float, float]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Vec4:
    def __new__(cls, x: float, y: float, z: float, w: float) -> Vec4: ...
    x: float
    y: float
    z: float
    w: float
    ZERO: Vec4
    def to_tuple(self) -> tuple[float, float, float, float]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Mat3:
    def __new__(cls, values: Sequence[float]) -> Mat3: ...
    @staticmethod
    def identity() -> Mat3: ...
    def to_list(self) -> list[float]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Vec3:
    def __new__(cls, x: float, y: float, z: float) -> Vec3: ...
    x: float
    y: float
    z: float
    X: Vec3
    Y: Vec3
    Z: Vec3
    ZERO: Vec3
    def to_tuple(self) -> tuple[float, float, float]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Quat:
    def __new__(cls, x: float, y: float, z: float, w: float) -> Quat: ...
    x: float
    y: float
    z: float
    w: float
    @staticmethod
    def identity() -> Quat: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Mat4:
    def __new__(cls, values: Sequence[float]) -> Mat4: ...
    @staticmethod
    def identity() -> Mat4: ...
    def to_list(self) -> list[float]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Rgba8:
    def __new__(cls, r: int, g: int, b: int, a: int) -> Rgba8: ...
    r: int
    g: int
    b: int
    a: int
    WHITE: Rgba8
    @staticmethod
    def opaque(r: int, g: int, b: int) -> Rgba8: ...
    def to_f32(self) -> list[float]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Aabb:
    def __new__(cls, min: Vec3, max: Vec3) -> Aabb: ...
    min: Vec3
    max: Vec3
    EMPTY: Aabb
    def is_empty(self) -> bool: ...
    def center(self) -> Vec3: ...
    def bounding_sphere(self) -> BoundingSphere: ...

@final
class BoundingSphere:
    def __new__(cls, center: Vec3, radius: float) -> BoundingSphere: ...
    center: Vec3
    radius: float

@final
class Projection:
    @staticmethod
    def perspective(fov_y: float, aspect: float, near: float = 0.1, far: float = 1000.0) -> Projection: ...
    @staticmethod
    def orthographic(height: float, aspect: float, near: float = 0.1, far: float = 1000.0) -> Projection: ...
    aspect: float
    kind: str
    def __repr__(self) -> str: ...

@final
class Camera:
    def __new__(cls, eye: Vec3, target: Vec3, up: Vec3, projection: Projection) -> Camera: ...
    eye: Vec3
    target: Vec3
    up: Vec3
    projection: Projection
    focus_distance: float
    @staticmethod
    def framing(bound: BoundingSphere, aspect: float) -> Camera: ...
    @staticmethod
    def framing_aabb(bound: Aabb, aspect: float) -> Camera: ...
    @staticmethod
    def look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4: ...
    def view_proj(self) -> Mat4: ...

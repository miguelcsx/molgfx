"""Camera primitives a host builds its own overlay and navigation from."""

import math
import unittest
from typing import Any, cast

import molframe

import molgfx

PDB = b"""\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N
ATOM      2  CA  ALA A   1       2.000   0.000   0.000  1.00  0.00           C
ATOM      3  N   GLY B   1      30.000  10.000   0.000  1.00  0.00           N
ATOM      4  CA  GLY B   1      34.000  10.000   0.000  1.00  0.00           C
END
"""

ONE_ATOM = b"""\
ATOM      1  C   ALA A   1       4.000   2.500   0.000  1.00  0.00           C
END
"""


def _camera(eye: tuple[float, float, float] = (0.0, 0.0, 20.0)) -> molgfx.Camera:
    return molgfx.Camera(position=eye, target=(0.0, 0.0, 0.0))


def _distance(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    return math.dist(a, b)


class ProjectionTests(unittest.TestCase):
    """Where a point lands, and the ray that lands on it."""

    def test_the_target_lands_at_the_centre_and_depth_is_the_distance(self) -> None:
        target = _camera().project((0.0, 0.0, 0.0), (200, 100))
        assert target is not None, "a point in front of the eye projects"
        x, y, depth = target
        self.assertAlmostEqual(x, 100.0, places=2)
        self.assertAlmostEqual(y, 50.0, places=2)
        self.assertAlmostEqual(depth, 20.0, places=3)

    def test_up_is_up_the_screen_and_a_point_behind_the_eye_is_absent(self) -> None:
        centre = _camera().project((0.0, 0.0, 0.0), (100, 100))
        above = _camera().project((0.0, 3.0, 0.0), (100, 100))
        assert centre is not None, "the origin projects"
        assert above is not None, "a point above the origin projects"
        self.assertLess(above[1], centre[1])
        self.assertIsNone(_camera().project((0.0, 0.0, 30.0), (100, 100)))

    def test_a_pixels_ray_passes_through_what_projects_to_that_pixel(self) -> None:
        ray = _camera().ray(30.0, 70.0, (200, 100))
        assert ray is not None, "a pixel inside the frame casts a ray"
        origin, direction = ray
        point = cast(
            "tuple[float, float, float]",
            tuple(o + 12.0 * d for o, d in zip(origin, direction, strict=True)),
        )
        landed = _camera().project(point, (200, 100))
        assert landed is not None, "a point on the ray in front of the eye projects"
        x, y, _ = landed
        self.assertAlmostEqual(x, 30.0, places=1)
        self.assertAlmostEqual(y, 70.0, places=1)
        self.assertAlmostEqual(math.hypot(*direction), 1.0, places=5)

    def test_it_agrees_with_where_the_renderer_draws_an_atom(self) -> None:
        scene = molgfx.Scene(molframe.read(ONE_ATOM, name="one.pdb"))
        scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
        view = _camera()
        scene.set_camera(view)
        size = (400, 300)
        try:
            image = molgfx.Renderer().render_image(scene, size=size)
        except molgfx.MolgfxError as error:
            if "no compatible GPU adapter" in str(error):
                self.skipTest("needs a graphics adapter; headless_render.py covers it")
            raise
        pixels = bytes(image.pixels())
        dark = [
            (index // 4 % size[0], index // 4 // size[0])
            for index in range(0, len(pixels), 4)
            if pixels[index] < 140 and pixels[index + 1] < 140 and pixels[index + 2] < 140
        ]
        self.assertGreater(len(dark), 200, "the atom is drawn")
        centroid = (
            sum(x for x, _ in dark) / len(dark),
            sum(y for _, y in dark) / len(dark),
        )
        projected = view.project((4.0, 2.5, 0.0), size)
        assert projected is not None, "a point in front of the eye projects"
        x, y, _ = projected
        self.assertLess(abs(centroid[0] - x), 3.0, (centroid, x, y))
        self.assertLess(abs(centroid[1] - y), 3.0, (centroid, x, y))


class FramingTests(unittest.TestCase):
    """Framing a selection reads it without changing the scene."""

    def scene(self) -> molgfx.Scene:
        scene = molgfx.Scene(molframe.read(PDB, name="two.pdb"))
        scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
        return scene

    def test_a_selection_has_bounds_and_a_camera_that_looks_at_it(self) -> None:
        scene = self.scene()
        bounds = scene.selection_bounds("chain B")
        assert bounds is not None, "chain B is selected"
        low, high = bounds
        self.assertEqual(low, (30.0, 10.0, 0.0))
        self.assertEqual(high, (34.0, 10.0, 0.0))
        framed = scene.frame("chain B", aspect=1.5)
        for got, want in zip(framed.target, (32.0, 10.0, 0.0), strict=True):
            self.assertAlmostEqual(got, want, places=3)

    def test_framing_leaves_the_scene_as_it_was(self) -> None:
        scene = self.scene()
        before = scene.revision
        scene.frame("chain B")
        self.assertEqual(scene.revision, before)

    def test_a_selection_that_picks_nothing_has_no_bounds_and_no_frame(self) -> None:
        scene = self.scene()
        self.assertIsNone(scene.selection_bounds("chain Z"))
        with self.assertRaises(molgfx.MolgfxError):
            scene.frame("chain Z")


class ControllerTests(unittest.TestCase):
    """The controllers a host drives with its own pointer and key events."""

    def test_an_arcball_drag_turns_the_view_about_the_target_at_a_constant_distance(
        self,
    ) -> None:
        controller = molgfx.ArcballController()
        view = _camera()
        controller.pointer_button("left", pressed=True, x=100.0, y=100.0, camera=view)
        turned = controller.pointer_move(100.0, 100.0, view)
        turned = controller.pointer_move(160.0, 100.0, turned)
        self.assertNotEqual(turned.position, view.position)
        self.assertAlmostEqual(_distance(turned.position, turned.target), 20.0, places=3)

    def test_moving_the_pointer_without_a_button_does_nothing(self) -> None:
        controller = molgfx.OrbitController()
        view = _camera()
        moved = controller.pointer_move(50.0, 50.0, view)
        moved = controller.pointer_move(90.0, 70.0, moved)
        self.assertEqual(moved.position, view.position)

    def test_scrolling_dollies_toward_the_target(self) -> None:
        view = _camera()
        closer = molgfx.ArcballController().scroll(2.0, view)
        self.assertLess(_distance(closer.position, closer.target), 20.0)

    def test_an_orbit_keeps_the_horizon_level(self) -> None:
        controller = molgfx.OrbitController()
        view = _camera()
        controller.pointer_button("left", pressed=True, x=0.0, y=0.0, camera=view)
        moved = controller.pointer_move(0.0, 0.0, view)
        moved = controller.pointer_move(80.0, 30.0, moved)
        self.assertEqual(moved.up, (0.0, 1.0, 0.0))

    def test_flying_forward_moves_the_eye_along_the_view(self) -> None:
        controller = molgfx.FlyController()
        view = _camera()
        controller.key("forward", pressed=True, camera=view)
        moved = controller.advance(view, 1.0, 5.0)
        self.assertAlmostEqual(moved.position[2], 15.0, places=3)

    def test_an_unknown_button_or_key_is_refused(self) -> None:
        view = _camera()
        # Neither spelling appears in the stub's enumerated sets, which is the
        # point: the binding has to reject a name it cannot enumerate, so the
        # value is held as text rather than dressed up as a valid literal.
        button: Any = "thumb"
        key: Any = "sideways"
        with self.assertRaises(ValueError):
            molgfx.ArcballController().pointer_button(
                button, pressed=True, x=0.0, y=0.0, camera=view
            )
        with self.assertRaises(ValueError):
            molgfx.FlyController().key(key, pressed=True, camera=view)


if __name__ == "__main__":
    unittest.main()

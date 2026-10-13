"""Scene API: batched placement, playback frame handling, defaults and reprs."""

import json
import unittest
from typing import Any

import molframe
import numpy as np

import molgfx

PDB = (
    b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \n"
    b"ATOM      2  CA  ALA A   2       4.000   0.000   0.000  1.00 10.00           C  \n"
    b"END\n"
)
ROTATION = ((0.0, -1.0, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0))
SIZE = (32, 32)


def _scene() -> molgfx.Scene:
    scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
    scene.add(molgfx.rep.spacefill())
    scene.set_camera(scene.frame("all"))
    return scene


def _shift(x: float) -> list[float]:
    return molgfx.place.from_rotation_translation(
        ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)), (x, 0.0, 0.0)
    )


class PlaceTests(unittest.TestCase):
    """Placing copies, one at a time and in batches."""

    def test_rotation_and_translation_match_the_matrix(self) -> None:
        by_matrix, by_parts = _scene(), _scene()
        matrix = molgfx.place.from_rotation_translation(ROTATION, (1.0, 2.0, 3.0))
        by_matrix.place(matrix)
        by_parts.place(rotation=ROTATION, translation=(1.0, 2.0, 3.0))
        self.assertEqual(by_matrix.selection_bounds("all"), by_parts.selection_bounds("all"))

    def test_place_needs_exactly_one_form(self) -> None:
        scene = _scene()
        with self.assertRaises(ValueError):
            scene.place()
        with self.assertRaises(ValueError):
            scene.place(_shift(1.0), rotation=ROTATION, translation=(0.0, 0.0, 0.0))

    def test_place_many_matches_placing_one_by_one(self) -> None:
        matrices = [_shift(10.0 * i) for i in range(1, 6)]
        many, single = _scene(), _scene()
        ids = many.place_many(matrices)
        for matrix in matrices:
            single.place(matrix)
        self.assertEqual(len(ids), 5)
        self.assertEqual(many.selection_bounds("all"), single.selection_bounds("all"))
        self.assertEqual(
            len(json.loads(many.to_json())["structures"]),
            len(json.loads(single.to_json())["structures"]),
        )

    def test_place_many_rejects_a_bad_matrix_and_changes_nothing(self) -> None:
        scene = _scene()
        before = scene.to_json()
        with self.assertRaises((ValueError, molgfx.MolgfxError)):
            scene.place_many([_shift(1.0), [0.0] * 3])
        self.assertEqual(scene.to_json(), before)

    def test_copies_render_like_the_slow_path(self) -> None:
        fast, slow = _scene(), _scene()
        fast.place(_shift(6.0))
        fast.place(_shift(12.0))
        slow.place_many([_shift(6.0), _shift(12.0)])
        renderer = molgfx.Renderer()
        self.assertEqual(
            renderer.render_image(fast, size=SIZE).pixels(),
            renderer.render_image(slow, size=SIZE).pixels(),
        )


class SeekTests(unittest.TestCase):
    """Playback seek with array and list frames."""

    TIMES = (0.0, 1.0, 2.0)

    def _frames(self, kind: str) -> list[Any]:
        frames = [np.array([[0, 0, 0], [4, y, 0]], dtype=np.float32) for y in (0.0, 3.0, 5.0)]
        if kind == "f64":
            return [f.astype(np.float64) for f in frames]
        if kind == "list":
            return [f.tolist() for f in frames]
        return frames

    def test_numpy_and_list_frames_render_the_same(self) -> None:
        renderer = molgfx.Renderer()
        seen = []
        for kind in ("f32", "f64", "list"):
            scene = _scene()
            play = molgfx.playback.attach(scene, self._frames(kind), self.TIMES)
            seen.append(play.render(renderer, times=(0.5, 1.5), size=SIZE))
        for other in seen[1:]:
            for a, b in zip(seen[0], other, strict=True):
                self.assertEqual(a.pixels(), b.pixels())

    def test_seek_is_repeatable_and_has_no_row_cache(self) -> None:
        scene = _scene()
        play = molgfx.playback.attach(scene, self._frames("f32"), self.TIMES)
        renderer = molgfx.Renderer()
        play.seek(0.5)
        first = renderer.render_image(scene, size=SIZE).pixels()
        play.seek(0.5)
        play.seek(1.5)
        play.seek(0.5)
        self.assertEqual(renderer.render_image(scene, size=SIZE).pixels(), first)
        self.assertFalse(hasattr(play, "_rows"))

    def test_frame_handles_are_cached(self) -> None:
        play = molgfx.playback.attach(_scene(), self._frames("f32"), self.TIMES)
        self.assertIs(play._frame(1), play._frame(1))

    def test_bad_frame_shape_is_rejected(self) -> None:
        with self.assertRaises((ValueError, TypeError)):
            molgfx.trajectory.frame(0, 0.0, np.zeros((2, 2), dtype=np.float32))


class DiscoverabilityTests(unittest.TestCase):
    """Defaults, reprs and aliases that make the API easier to find."""

    def test_rep_target_defaults_to_all(self) -> None:
        scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
        scene.add(molgfx.rep.ball_and_stick())
        scene.add(molgfx.rep.spacefill(target="all"))
        reps = json.loads(scene.to_json())["representations"]
        self.assertEqual(len(reps), 2)

    def test_overlay_reprs_and_getters(self) -> None:
        origin = molgfx.annotation.world((0.0, 0.0, 0.0))
        other = molgfx.annotation.world((1.0, 0.0, 0.0))
        label = molgfx.annotation.label(anchor=origin, text="site")
        measure = molgfx.measurement.distance(origin, other)
        self.assertEqual(origin.kind, "world")
        self.assertEqual(origin.position, (0.0, 0.0, 0.0))
        self.assertEqual(label.text, "site")
        self.assertEqual(measure.kind, "distance")
        self.assertEqual(len(measure.anchors), 2)
        for obj, word in ((origin, "world"), (label, "site"), (measure, "distance")):
            self.assertIn(word, repr(obj))
            self.assertNotIn("object at 0x", repr(obj))

    def test_image_quality_alias(self) -> None:
        image = molgfx.Renderer().render_image(_scene(), size=SIZE)
        self.assertEqual(image.quality, json.loads(image.quality_json()))


if __name__ == "__main__":
    unittest.main()

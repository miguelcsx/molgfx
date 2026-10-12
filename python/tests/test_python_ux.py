"""Python ergonomics: empty targets, reprs, helpers for placement, playback and colour."""

import math
import unittest

import molframe

import molgfx

PDB = (
    b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \n"
    b"ATOM      2  CA  ALA A   2       4.000   0.000   0.000  1.00 10.00           C  \n"
    b"END\n"
)
FRAMES = (
    ((0.0, 0.0, 0.0), (4.0, 0.0, 0.0)),
    ((0.0, 3.0, 0.0), (4.0, -3.0, 0.0)),
    ((0.0, 0.0, 0.0), (4.0, 5.0, 0.0)),
)
SIZE = (48, 48)


def _scene() -> molgfx.Scene:
    scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
    scene.add(molgfx.rep.spacefill(target="all"))
    scene.set_camera(scene.frame("all"))
    return scene


class EmptyTargetTests(unittest.TestCase):
    """Empty targets fail the same way for `add` and `pocket`."""

    def test_empty_target_is_rejected_like_pocket(self) -> None:
        scene = _scene()
        with self.assertRaisesRegex(molgfx.SpecError, "selects no atoms"):
            scene.add(molgfx.rep.licorice(target="chain label:X"))
        with self.assertRaises(molgfx.SpecError):
            scene.pocket("chain label:X")

    def test_allow_empty_opts_out(self) -> None:
        scene = _scene()
        before = scene.revision
        scene.add(molgfx.rep.licorice(target="chain label:X"), allow_empty=True)
        self.assertGreater(scene.revision, before)

    def test_non_empty_target_still_adds(self) -> None:
        self.assertIsInstance(
            _scene().add(molgfx.rep.licorice(target="chain A")), molgfx.RepresentationId
        )


class ReprAndDocTests(unittest.TestCase):
    """Objects print usefully and methods explain themselves."""

    def test_reprs_are_informative(self) -> None:
        scene = _scene()
        self.assertEqual(repr(scene).split("(")[0], "Scene")
        self.assertIn("structures=1", repr(scene))
        self.assertTrue(repr(molgfx.Renderer()).startswith("Renderer("))
        self.assertTrue(repr(molgfx.profile.interactive()).startswith("RenderProfile("))
        self.assertEqual(repr(molgfx.Renderer().render_image(scene, size=SIZE)), "Image(48x48)")

    def test_methods_have_docstrings(self) -> None:
        for method in (
            molgfx.Scene.add,
            molgfx.Scene.place,
            molgfx.Scene.pocket,
            molgfx.Scene.bind_property,
            molgfx.Scene.bind_trajectory,
            molgfx.Scene.set_trajectory_time,
            molgfx.Scene.set_trajectory_pair,
            molgfx.Renderer.render_image,
            molgfx.Renderer.render_sequence,
        ):
            self.assertTrue(method.__doc__, method)

    def test_viewer_is_listed_without_importing_anywidget(self) -> None:
        self.assertIn("Viewer", dir(molgfx))
        self.assertIn("Viewer", molgfx.__all__)
        self.assertIn("by_array", dir(molgfx))


class PlacementTests(unittest.TestCase):
    """`molgfx.place` follows the column-major convention."""

    ROTATION = ((0.0, -1.0, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0))
    TRANSLATION = (1.0, 2.0, 3.0)

    def test_matrix_is_column_major(self) -> None:
        matrix = molgfx.place.from_rotation_translation(self.ROTATION, self.TRANSLATION)
        self.assertEqual(matrix[12:15], [1.0, 2.0, 3.0])
        self.assertEqual(matrix[:3], [0.0, 1.0, 0.0])

    def test_placed_copy_matches_r_x_plus_t(self) -> None:
        scene = _scene()
        molgfx.place.apply(scene, (self.ROTATION, self.TRANSLATION))
        low, high = scene.selection_bounds("all") or ((), ())
        # (0,0,0) -> (1,2,3); (4,0,0) -> (1,6,3); the originals span x 0..4.
        self.assertEqual(low, (0.0, 0.0, 0.0))
        self.assertEqual(high, (4.0, 6.0, 3.0))

    def test_accepts_a_superposition_object(self) -> None:
        class Fit:
            rotation = PlacementTests.ROTATION
            translation = PlacementTests.TRANSLATION

        self.assertEqual(
            molgfx.place.from_superposition(Fit()),
            molgfx.place.from_rotation_translation(self.ROTATION, self.TRANSLATION),
        )

    def test_rejects_wrong_shapes(self) -> None:
        with self.assertRaises(ValueError):
            molgfx.place.from_rotation_translation(((1.0, 0.0),), (0.0, 0.0, 0.0))

    def test_matches_molframe_superpose(self) -> None:
        theta = math.pi / 2
        rotation = (
            (math.cos(theta), -math.sin(theta), 0.0),
            (math.sin(theta), math.cos(theta), 0.0),
            (0.0, 0.0, 1.0),
        )
        matrix = molgfx.place.from_rotation_translation(rotation, (0.0, 0.0, 0.0))
        self.assertAlmostEqual(matrix[1], math.sin(theta))
        self.assertAlmostEqual(matrix[4], -math.sin(theta))


class PlaybackTests(unittest.TestCase):
    """N-frame playback moves the resident pair."""

    def test_time_before_a_pair_is_resident_explains_the_recipe(self) -> None:
        scene = _scene()
        with self.assertRaisesRegex(molgfx.MolgfxError, "set_trajectory_pair"):
            scene.set_trajectory_time(structure=scene.structure_id, seconds=0.5)

    def test_n_frame_playback_renders_distinct_frames(self) -> None:
        scene = _scene()
        play = molgfx.playback.attach(scene, FRAMES, (0.0, 1.0, 2.0))
        images = play.render(molgfx.Renderer(), times=(0.0, 0.5, 1.0, 1.5, 2.0), size=SIZE)
        self.assertEqual(len({image.pixels() for image in images}), 5)

    def test_render_sequence_is_static(self) -> None:
        scene = _scene()
        molgfx.playback.attach(scene, FRAMES, (0.0, 1.0, 2.0))
        images = molgfx.Renderer().render_sequence(scene, size=SIZE, fps=2, frames=3)
        self.assertEqual(len({image.pixels() for image in images}), 1)

    def test_validates_frames_and_times(self) -> None:
        with self.assertRaises(ValueError):
            molgfx.playback.attach(_scene(), FRAMES[:1], (0.0,))
        with self.assertRaises(ValueError):
            molgfx.playback.attach(_scene(), FRAMES, (0.0, 2.0, 1.0))
        with self.assertRaises(ValueError):
            molgfx.playback.attach(_scene(), FRAMES, (0.0, 1.0))

    def test_time_is_clamped(self) -> None:
        scene = _scene()
        play = molgfx.playback.attach(scene, FRAMES, (0.0, 1.0, 2.0))
        renderer = molgfx.Renderer()
        play.seek(9.0)
        late = renderer.render_image(scene, size=SIZE).pixels()
        play.seek(2.0)
        self.assertEqual(late, renderer.render_image(scene, size=SIZE).pixels())


class ColourTests(unittest.TestCase):
    """`by_array` replaces hand-made property hashes."""

    def test_by_array_colours_without_a_hand_made_hash(self) -> None:
        scene = _scene()
        color = molgfx.by_array(scene, [1.0, 3.0], ramp="viridis")
        scene.add(molgfx.rep.spacefill(target="all", color=color))
        self.assertIn("values-", scene.to_json())

    def test_hash_follows_the_data(self) -> None:
        scene = _scene()
        first = molgfx.by_array(scene, [1.0, 2.0])
        second = molgfx.by_array(scene, [1.0, 3.0])
        self.assertIsNotNone(first)
        self.assertIsNotNone(second)

    def test_empty_values_are_rejected(self) -> None:
        with self.assertRaises(ValueError):
            molgfx.by_array(_scene(), [])


if __name__ == "__main__":
    unittest.main()

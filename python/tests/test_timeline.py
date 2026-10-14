"""Typed physical time: molframe picoseconds, unit-checked attach and a multi-rate timeline."""

import math
import unittest
from dataclasses import dataclass
from typing import Any

import molframe

import molgfx
from molgfx.time import PlaybackMode, TimeUnit

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


@dataclass(frozen=True)
class _Trajectory:
    """A stand-in for a molframe trajectory: positions plus picosecond times."""

    times: tuple[float, ...]
    positions: tuple[tuple[tuple[float, float, float], ...], ...] = FRAMES


def _scene() -> molgfx.Scene:
    scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
    scene.add(molgfx.rep.spacefill(target="all"))
    scene.set_camera(scene.frame("all"))
    return scene


class AttachUnitTests(unittest.TestCase):
    """Frame times cross the boundary once, typed and finite."""

    def test_a_molframe_picosecond_trajectory_reads_back_in_nanoseconds(self) -> None:
        scene = _scene()
        molgfx.playback.attach_trajectory(scene, _Trajectory((0.0, 20000.0, 40000.0)))
        timeline = molgfx.Timeline(scene)
        timeline.fit_track(
            scene.structure_id, range=(0.0, 20.0), unit=TimeUnit.NANOSECOND, seconds=10.0
        )
        timeline.apply(6.2)
        ((value, unit),) = timeline.readings()
        self.assertAlmostEqual(value, 12.4, places=9)
        self.assertIs(unit, TimeUnit.NANOSECOND)

    def test_the_picosecond_times_are_stored_as_seconds_exactly_once(self) -> None:
        play = molgfx.playback.attach_trajectory(_scene(), _Trajectory((0.0, 250.0, 500.0)))
        for stamp, seconds in zip(play.times, (0.0, 2.5e-10, 5.0e-10), strict=True):
            self.assertAlmostEqual(stamp, seconds, delta=1.0e-22)

    def test_an_explicit_unit_reinterprets_the_trajectory_times(self) -> None:
        play = molgfx.playback.attach_trajectory(
            _scene(), _Trajectory((0.0, 1.0, 2.0)), unit=TimeUnit.NANOSECOND
        )
        self.assertAlmostEqual(play.times[2], 2.0e-9, delta=1.0e-21)

    def test_a_nan_or_infinite_time_is_refused_naming_its_frame(self) -> None:
        for bad in (math.nan, math.inf, -math.inf):
            with self.subTest(time=bad):
                with self.assertRaisesRegex(ValueError, "frame 1"):
                    molgfx.playback.attach_trajectory(_scene(), _Trajectory((0.0, bad, 500.0)))
                with self.assertRaisesRegex(ValueError, "frame 1"):
                    molgfx.playback.attach(_scene(), FRAMES, (0.0, bad, 2.0), unit=TimeUnit.SECOND)

    def test_an_unknown_unit_is_refused_at_the_api(self) -> None:
        # The refusal is the point: free text is outside the typed set, so it is
        # held as `Any` and the binding must reject it.
        unknown: Any = "nanoseconds"
        scene = _scene()
        with self.assertRaisesRegex(ValueError, "unknown time unit"):
            molgfx.playback.attach(scene, FRAMES, (0.0, 1.0, 2.0), unit=unknown)
        with self.assertRaisesRegex(ValueError, "unknown time unit"):
            molgfx.playback.attach_trajectory(scene, _Trajectory((0.0, 1.0, 2.0)), unit=unknown)
        with self.assertRaisesRegex(ValueError, "unknown time unit"):
            molgfx.Timeline(scene).fit_track(
                scene.structure_id, range=(0.0, 1.0), unit=unknown, seconds=1.0
            )
        not_text: Any = 5
        with self.assertRaises(ValueError):
            molgfx.playback.attach(scene, FRAMES, (0.0, 1.0, 2.0), unit=not_text)

    def test_the_unit_is_required_when_attaching_bare_times(self) -> None:
        attach: Any = molgfx.playback.attach
        with self.assertRaises(TypeError):
            attach(_scene(), FRAMES, (0.0, 1.0, 2.0))


class TimelineTests(unittest.TestCase):
    """Tracks at different physical scales share one playback clock."""

    def _two_tracks(self) -> tuple[molgfx.Timeline, molgfx.Scene]:
        scene = _scene()
        first = scene.structure_id
        second = scene.add_structure(molframe.read(PDB, name="u.pdb"))
        molgfx.playback.attach(scene, FRAMES, (0.0, 100.0, 200.0), unit=TimeUnit.NANOSECOND)
        molgfx.playback.attach(
            scene, FRAMES, (0.0, 100.0, 200.0), unit=TimeUnit.MICROSECOND, structure=second
        )
        timeline = molgfx.Timeline(scene)
        timeline.fit_track(first, range=(0.0, 100.0), unit=TimeUnit.NANOSECOND, seconds=10.0)
        timeline.fit_track(second, range=(0.0, 100.0), unit=TimeUnit.MICROSECOND, seconds=10.0)
        return timeline, scene

    def test_a_ns_and_a_us_track_fitted_to_ten_seconds_report_their_own_readings(self) -> None:
        timeline, _ = self._two_tracks()
        self.assertEqual(len(timeline), 2)
        for playback, expected in ((0.0, 0.0), (2.5, 25.0), (10.0, 100.0)):
            timeline.apply(playback)
            readings = timeline.readings()
            self.assertEqual(
                [unit for _, unit in readings], [TimeUnit.NANOSECOND, TimeUnit.MICROSECOND]
            )
            for value, _ in readings:
                self.assertAlmostEqual(value, expected, places=6)
            self.assertEqual(timeline.seconds, playback)

    def test_a_dilated_track_advances_at_its_pace_and_loops(self) -> None:
        scene = _scene()
        molgfx.playback.attach(scene, FRAMES, (0.0, 100.0, 200.0), unit=TimeUnit.NANOSECOND)
        timeline = molgfx.Timeline(scene)
        track = timeline.dilate_track(
            scene.structure_id,
            range=(0.0, 100.0),
            unit=TimeUnit.NANOSECOND,
            per_second=10.0,
            mode=PlaybackMode.LOOP,
        )
        timeline.apply(12.0)
        value, unit = timeline.reading(track)
        self.assertAlmostEqual(value, 20.0, places=6)
        self.assertIs(unit, TimeUnit.NANOSECOND)

    def test_a_display_unit_overrides_the_fitted_one(self) -> None:
        scene = _scene()
        molgfx.playback.attach(scene, FRAMES, (0.0, 100.0, 200.0), unit=TimeUnit.NANOSECOND)
        timeline = molgfx.Timeline(scene)
        timeline.fit_track(
            scene.structure_id,
            range=(0.0, 100.0),
            unit=TimeUnit.NANOSECOND,
            seconds=10.0,
            display_unit=TimeUnit.PICOSECOND,
        )
        timeline.apply(5.0)
        ((value, unit),) = timeline.readings()
        self.assertAlmostEqual(value, 50000.0, places=3)
        self.assertIs(unit, TimeUnit.PICOSECOND)

    def test_a_track_outside_the_resident_pair_is_refused(self) -> None:
        scene = _scene()
        molgfx.playback.attach(scene, FRAMES, (0.0, 100.0, 200.0), unit=TimeUnit.NANOSECOND)
        with self.assertRaises(molgfx.MolgfxError):
            molgfx.Timeline(scene).fit_track(
                scene.structure_id, range=(0.0, 150.0), unit=TimeUnit.NANOSECOND, seconds=1.0
            )

    def test_a_timeline_moves_the_scene_through_its_track(self) -> None:
        timeline, scene = self._two_tracks()
        renderer = molgfx.Renderer()
        timeline.apply(0.0)
        start = renderer.render_image(scene, size=(32, 32)).pixels()
        timeline.apply(10.0)
        self.assertNotEqual(start, renderer.render_image(scene, size=(32, 32)).pixels())


if __name__ == "__main__":
    unittest.main()

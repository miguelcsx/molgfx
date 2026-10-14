"""The notebook viewer drives the browser transport and serves trajectory chunks."""

import struct
import unittest
from collections.abc import Callable
from typing import Any
from unittest import mock

import molframe
from test_timeline import FRAMES, PDB, _Trajectory

import molgfx
from molgfx._engine import Playback
from molgfx.time import PlaybackMode, TimeUnit


def _viewer() -> tuple[molgfx.Viewer, Playback]:
    scene = molgfx.Scene(molframe.read(PDB, name="t.pdb"))
    scene.add(molgfx.rep.spacefill(target="all"))
    playback = molgfx.playback.attach_trajectory(scene, _Trajectory((0.0, 20000.0, 40000.0)))
    viewer = molgfx.Viewer(scene)
    return viewer, playback


class ViewerTransportTests(unittest.TestCase):
    """Commands are plain messages; replies ride the chunk trait."""

    def setUp(self) -> None:
        self.viewer, self.playback = _viewer()
        self.addCleanup(self.viewer.close)

    def _sent(self, action: Callable[[], object]) -> list[object]:
        with mock.patch.object(self.viewer, "send") as send:
            action()
        return [call.args[0] for call in send.call_args_list]

    def test_commands_send_one_message_each(self) -> None:
        self.assertEqual(self._sent(self.viewer.play), [{"type": "play"}])
        self.assertEqual(self._sent(self.viewer.pause), [{"type": "pause"}])
        self.assertEqual(
            self._sent(lambda: self.viewer.seek(1.5)), [{"type": "seek", "seconds": 1.5}]
        )
        self.assertEqual(
            self._sent(lambda: self.viewer.set_rate(2)), [{"type": "set_rate", "rate": 2.0}]
        )
        self.assertEqual(
            self._sent(lambda: self.viewer.set_loop(PlaybackMode.PING_PONG)),
            [{"type": "set_loop", "mode": "pingpong"}],
        )

    def test_set_loop_refuses_free_text(self) -> None:
        # The refusal is the point: a bare string must not reach the browser.
        spelled: Any = "loop"
        with mock.patch.object(self.viewer, "send") as send, self.assertRaises(TypeError):
            self.viewer.set_loop(spelled)
        send.assert_not_called()

    def test_non_finite_numbers_are_refused(self) -> None:
        with self.assertRaises(ValueError):
            self.viewer.seek(float("nan"))
        with self.assertRaises(ValueError):
            self.viewer.set_rate(float("inf"))

    def test_timechange_updates_the_read_only_time(self) -> None:
        self.assertIsNone(self.viewer.time)
        self.viewer._on_message(
            self.viewer,
            {"type": "timechange", "seconds": 2.5, "readouts": [0, 12.0, "us", 0.25]},
            [],
        )
        time = self.viewer.time
        assert time is not None
        self.assertEqual(time.seconds, 2.5)
        self.assertEqual(time.readouts[0].unit, TimeUnit.MICROSECOND)
        # Assignment is refused on purpose; typed as Any so the checker lets the test try it.
        loose: Any = self.viewer
        with self.assertRaises(AttributeError):
            loose.time = None

    def test_a_chunk_request_is_answered_through_the_chunk_trait(self) -> None:
        self.viewer.stream_trajectory(self.playback)
        structure = str(self.playback.structure.value)
        self.viewer._on_message(
            self.viewer,
            {"type": "chunk_request", "structure": structure, "from_seconds": 2.5e-8, "count": 2},
            [],
        )
        serial, name, rows = self.viewer.trajectory_chunk
        self.assertEqual(name, structure)
        self.assertEqual([row[1] for row in rows], ["1", "2"])
        self.assertAlmostEqual(rows[0][2], 2.0e-8)
        self.assertEqual(struct.unpack("<6f", rows[0][3]), (0.0, 3.0, 0.0, 4.0, -3.0, 0.0))
        self.assertEqual(len({row[0] for row in rows}), 2)
        self.assertGreater(serial, 1)

    def test_streaming_and_sampling_use_the_chunk_path(self) -> None:
        self.viewer.stream_trajectory(self.playback)
        first = self.viewer.trajectory_chunk
        self.assertEqual([row[1] for row in first[2]], ["0", "1", "2"])
        self.viewer.sample_trajectory(0.0)
        second = self.viewer.trajectory_chunk
        self.assertGreater(second[0], first[0])
        self.assertEqual(self.viewer.trajectory_sample, ("", 0.0))
        self.assertEqual(len(FRAMES), len(second[2]))

    def test_unknown_or_malformed_requests_are_ignored(self) -> None:
        before = self.viewer.trajectory_chunk
        for message in (
            {"type": "chunk_request", "structure": "nope", "from_seconds": 0, "count": 1},
            {"type": "chunk_request", "structure": 3, "from_seconds": 0, "count": 1},
            {"type": "other"},
        ):
            self.viewer._on_message(self.viewer, message, [])
        self.assertEqual(self.viewer.trajectory_chunk, before)

    def test_a_closed_viewer_refuses_playback_commands(self) -> None:
        self.viewer.close()
        with self.assertRaises(RuntimeError):
            self.viewer.play()


if __name__ == "__main__":
    unittest.main()

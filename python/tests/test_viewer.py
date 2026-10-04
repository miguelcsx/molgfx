"""Consumer-visible Session, widget transport and resource lifecycle boundaries."""

import json
import unittest
import weakref
from typing import TYPE_CHECKING

from test_commands import _structure

import molgfx
from molgfx import Command, CommandError, Scene, Session, Viewer

if TYPE_CHECKING:
    from collections.abc import Callable


class _Subscriber:
    """Record real scene publication with an optional lifecycle mutation."""

    def __init__(self) -> None:
        self.received: list[str] = []
        self.action: Callable[[], object] | None = None

    def on_patch(self, patch: str) -> None:
        """Record the committed patch before any requested subscriber action."""
        self.received.append(patch)
        if self.action is not None:
            self.action()


class ViewerTests(unittest.TestCase):
    """Exercise the real extension and generated runtime without a notebook server."""

    def make_viewer(self, source: Session | Scene | None = None) -> Viewer:
        """Close every widget deterministically, including on assertion failures."""
        viewer = Viewer(_structure() if source is None else source)
        self.addCleanup(viewer.close)
        return viewer

    def test_structure_auto_and_existing_scene_and_session_are_distinct(self) -> None:
        automatic = self.make_viewer()
        self.assertTrue(automatic.session.layers)
        scene = Scene(_structure())
        authored = self.make_viewer(scene)
        self.assertIs(authored.scene, scene)
        self.assertEqual(authored.session.layers, {})
        session = Session(scene)
        shared = self.make_viewer(session)
        self.assertIs(shared.session, session)

    def test_conveniences_commit_through_the_same_session(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        viewer.show("cartoon", "protein")
        viewer.color("chain", "protein")
        viewer.hide("cartoon")
        viewer.focus("resname HEM")
        viewer.select("resname HEM")
        self.assertEqual(viewer.session.selections, {})
        self.assertEqual(viewer.scene.revision, viewer.revision)
        self.assertEqual(viewer.patch_sequence, 5)
        self.assertEqual(json.loads(viewer.scene_patch)["operations"][0]["op"], "set_interaction")
        self.assertIs(molgfx.Viewer, Viewer)

    def test_direction_wedges_reach_the_shared_browser_scene_patch(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        viewer.execute("show cartoon direction_wedges=true, protein")
        self.assertEqual(viewer.patch_sequence, 1)
        self.assertIn('"direction_wedges":true', viewer.scene_patch)
        self.assertIn('"direction_wedges":true', viewer.scene.to_json())
        viewer.session.undo()
        self.assertEqual(viewer.patch_sequence, 2)
        self.assertNotIn('"direction_wedges":true', viewer.scene.to_json())
        before = viewer.patch_sequence
        with self.assertRaises(CommandError):
            viewer.execute("show cartoon direction_wedges=yes, protein")
        self.assertEqual(viewer.patch_sequence, before)

    def test_failed_command_does_not_publish_or_mutate(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        before = viewer.scene.to_json(), viewer.patch_sequence, viewer.session.to_json()
        with self.assertRaises(CommandError):
            viewer.execute("show cartoon, protein; hide @missing")
        self.assertEqual(
            before, (viewer.scene.to_json(), viewer.patch_sequence, viewer.session.to_json())
        )

    def test_shared_session_edit_is_sent_once_to_each_open_viewer(self) -> None:
        session = Session(_structure())
        first = self.make_viewer(session)
        second = self.make_viewer(session)
        result = session.execute(Command.show("spacefill", "all"))
        self.assertEqual((first.patch_sequence, second.patch_sequence), (1, 1))
        assert result.patch is not None
        self.assertEqual(first.scene_patch, result.patch.to_json())
        first.close()
        first.close()
        session.execute(Command.hide("spacefill"))
        self.assertEqual((first.patch_sequence, second.patch_sequence), (1, 2))
        self.assertIsNone(first._subscription)
        first.sync_request += 1
        self.assertEqual(first.patch_sequence, 1)
        with self.assertRaises(RuntimeError):
            first.show("cartoon", "protein")

    def test_structure_announcement_and_resync_replace_the_full_snapshot(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        viewer.scene.add_structure(_structure())
        self.assertEqual(viewer.structure_ids, [1, 2])
        self.assertEqual(viewer.structure_sources, [1, 1])
        self.assertEqual(viewer.structure_payloads[1], b"")
        self.assertEqual(viewer.scene_spec, viewer.scene.to_json())
        viewer.show("spacefill", "all", structure="s1")
        viewer.sync_request += 1
        self.assertEqual(viewer.scene_spec, viewer.scene.to_json())
        self.assertEqual(viewer.scene_patch, "")
        self.assertEqual(viewer.revision, viewer.scene.revision)

    def test_rich_pick_is_not_reconstructed_as_a_residue_selection(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        event: dict[str, object] = {
            "kind": "pick",
            "eventId": "one",
            "sceneGeneration": 1,
            "result": json.dumps(
                {
                    "pick": "atom",
                    "structure": 2,
                    "dataset": 4,
                    "chunk": 5,
                    "topology_revision": 8,
                    "atom_index": 1,
                }
            ),
            "modifiers": {"shift": False, "alt": False, "ctrl": False, "meta": False},
        }
        viewer.interaction_event = event
        self.assertEqual(viewer.interaction_event, event)
        self.assertEqual(viewer.session.selections, {})
        self.assertEqual(viewer.scene.revision, 0)

    def test_selection_is_one_undoable_edit_and_escape_clear_is_idempotent(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        selected = viewer.select("resname HEM")
        self.assertEqual(selected.revision, viewer.revision)
        self.assertEqual(viewer.patch_sequence, 1)
        snapshot = viewer.scene.to_json()
        viewer.session.undo()
        viewer.session.redo()
        self.assertEqual(viewer.patch_sequence, 3)
        # Redo preserves the full selected state while advancing the revision.
        restored = json.loads(viewer.scene.to_json())
        expected = json.loads(snapshot)
        restored.pop("revision")
        expected.pop("revision")
        self.assertEqual(restored, expected)
        viewer.interaction_event = {"kind": "clear", "eventId": "clear-1", "sceneGeneration": 1}
        self.assertEqual(viewer.patch_sequence, 4)
        viewer.interaction_event = {"kind": "clear", "eventId": "clear-1", "sceneGeneration": 2}
        self.assertEqual(viewer.patch_sequence, 4)
        viewer.close()
        viewer.interaction_event = {"kind": "clear", "eventId": "clear-2", "sceneGeneration": 2}
        self.assertEqual(viewer.patch_sequence, 4)

    def test_invalid_selection_does_not_change_the_scene_or_history(self) -> None:
        viewer = self.make_viewer(Session(_structure()))
        viewer.select("resname HEM")
        before = viewer.scene.to_json(), viewer.session.to_json(), viewer.patch_sequence
        with self.assertRaises(CommandError):
            viewer.select("resname (")
        self.assertEqual(
            before, (viewer.scene.to_json(), viewer.session.to_json(), viewer.patch_sequence)
        )

    def test_a_subscriber_can_unsubscribe_itself_during_session_publication(self) -> None:
        session = Session(_structure())
        scene = session.scene
        subscriber = _Subscriber()
        reference = weakref.WeakMethod(subscriber.on_patch)
        subscriber.action = lambda: scene.unsubscribe(reference)
        scene.subscribe(reference)
        session.execute(Command.set_selection("all"))
        session.execute(Command.set_selection(None))
        self.assertEqual(len(subscriber.received), 1)

    def test_a_new_subscriber_begins_with_the_next_committed_patch(self) -> None:
        session = Session(_structure())
        scene = session.scene
        first = _Subscriber()
        second = _Subscriber()
        reference = weakref.WeakMethod(second.on_patch)
        first.action = lambda: scene.subscribe(reference)
        scene.subscribe(weakref.WeakMethod(first.on_patch))
        session.execute(Command.set_selection("all"))
        self.assertEqual(second.received, [])
        first.action = None
        session.execute(Command.set_selection(None))
        self.assertEqual(len(second.received), 1)
        self.assertEqual(len(first.received), 2)

    def test_a_failing_subscriber_remains_registered_for_the_next_patch(self) -> None:
        scene = Scene(_structure())
        subscriber = _Subscriber()

        def fail() -> None:
            message = "subscriber failure"
            raise ValueError(message)

        subscriber.action = fail
        scene.subscribe(weakref.WeakMethod(subscriber.on_patch))
        with self.assertRaisesRegex(ValueError, "subscriber failure"):
            scene.set_interaction(channel="selected", target="all")
        with self.assertRaisesRegex(ValueError, "subscriber failure"):
            scene.set_interaction(channel="selected", target=None)
        self.assertEqual(len(subscriber.received), 2)

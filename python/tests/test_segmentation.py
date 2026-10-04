"""Categorical-grid authoring, exact labels and undo through the facade."""

import json
import unittest
from typing import Any, Literal

import molgfx

AFFINE = (1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 4.0, 0.0, 0.0, 1.0)


class SegmentationTests(unittest.TestCase):
    """Specifications remain portable while categorical samples remain runtime data."""

    def test_typed_specification_round_trips_without_bulk_labels(self) -> None:
        specification = molgfx.segmentation.volume(
            source=molgfx.data.source("labels"),
            dimensions=(2, 2, 2),
            voxel_to_world=AFFINE,
            styles=[molgfx.segmentation.style(2**32 - 1, color=(255, 0, 0), opacity=0.5)],
        )
        self.assertIsInstance(specification, molgfx.Segmentation)
        encoded = specification.to_json()
        self.assertEqual(molgfx.Segmentation.from_json(encoded).to_json(), encoded)
        scene = molgfx.Scene()
        identity = scene.add(specification)
        self.assertIsInstance(identity, molgfx.SegmentationId)
        key = f"{identity.index}:{identity.generation}"
        scene.bind_segmentation(identity, [0, 2**32 - 1] * 4)
        document = json.loads(scene.to_json())
        self.assertEqual(document["segmentations"][key]["voxel_to_world"], list(AFFINE))
        self.assertEqual(document["segmentations"][key]["styles"][0]["label"], 2**32 - 1)
        self.assertNotIn("labels", document["segmentations"][key])
        self.assertNotIn("volume", document["segmentations"][key])

    def test_surface_and_direct_presentations_round_trip(self) -> None:
        modes: tuple[Literal["surface", "direct"], ...] = ("surface", "direct")
        for mode in modes:
            specification = molgfx.segmentation.volume(
                source=molgfx.data.source("labels"),
                dimensions=(2, 2, 2),
                styles=[],
                presentation=mode,
            )
            encoded = specification.to_json()
            self.assertEqual(json.loads(encoded)["presentation"], mode)
            self.assertEqual(molgfx.Segmentation.from_json(encoded).to_json(), encoded)
        # Deliberately bypass static enum checking to exercise binding refusal.
        invalid: Any = "unknown"
        with self.assertRaises(ValueError):
            molgfx.segmentation.volume(
                source=molgfx.data.source("labels"),
                dimensions=(2, 2, 2),
                styles=[],
                presentation=invalid,
            )

    def test_style_replacement_and_removal_use_exact_generational_identity(self) -> None:
        scene = molgfx.Scene()
        item = molgfx.segmentation.volume(
            source=molgfx.data.source("labels"),
            dimensions=(2, 2, 2),
            styles=[molgfx.segmentation.style(42)],
        )
        identity = scene.add(item)
        scene.bind_segmentation(identity, [42] * 8)
        scene.set_segment_styles(identity, [])
        key = f"{identity.index}:{identity.generation}"
        self.assertEqual(json.loads(scene.to_json())["segmentations"][key]["styles"], [])
        scene.remove_segmentation(identity)
        replacement = scene.add(item)
        self.assertNotEqual(
            (identity.index, identity.generation), (replacement.index, replacement.generation)
        )
        with self.assertRaises((molgfx.MolgfxError, ValueError)):
            scene.bind_segmentation(identity, [42] * 8)
        with self.assertRaises(molgfx.MolgfxError):
            scene.set_segment_styles(identity, [])

    def test_label_ingestion_rejects_invalid_shape_and_unsigned_overflow(self) -> None:
        scene = molgfx.Scene()
        identity = scene.add(
            molgfx.segmentation.volume(
                source=molgfx.data.source("labels"), dimensions=(2, 2, 2), styles=[]
            )
        )
        with self.assertRaises(molgfx.MolgfxError):
            scene.bind_segmentation(identity, [1])
        for labels in ([-1] * 8, [2**32] * 8):
            with self.assertRaises(OverflowError):
                scene.bind_segmentation(identity, labels)

    def test_segment_commands_restore_styles_and_declarations_with_undo(self) -> None:
        scene = molgfx.Scene()
        session = molgfx.Session(scene)
        spec = molgfx.segmentation.volume(
            source=molgfx.data.source("labels"),
            dimensions=(2, 2, 2),
            styles=[molgfx.segmentation.style(42)],
        )
        session.execute("segment " + spec.to_json())
        authored = json.loads(scene.to_json())["segmentations"]
        key = next(iter(authored))
        session.execute(f"segment style {key} []")
        self.assertEqual(json.loads(scene.to_json())["segmentations"][key]["styles"], [])
        session.undo()
        self.assertEqual(json.loads(scene.to_json())["segmentations"], authored)
        session.undo()
        self.assertEqual(json.loads(scene.to_json())["segmentations"], {})
        session.redo()
        self.assertEqual(json.loads(scene.to_json())["segmentations"], authored)

    def test_invalid_style_edits_leave_the_scene_unchanged(self) -> None:
        scene = molgfx.Scene()
        identity = scene.add(
            molgfx.segmentation.volume(
                source=molgfx.data.source("labels"), dimensions=(2, 2, 2), styles=[]
            )
        )
        before = scene.to_json()
        with self.assertRaises(molgfx.MolgfxError):
            scene.set_segment_styles(identity, [molgfx.segmentation.style(1, opacity=2.0)])
        self.assertEqual(scene.to_json(), before)

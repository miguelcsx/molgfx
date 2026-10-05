"""Native point scenes retain compact arrays without molecular metadata."""

import unittest

import numpy as np

from molgfx import Camera, MolgfxError, PointCloud, Scene, color


class PointCloudTests(unittest.TestCase):
    """Array groups are validated before allocating renderer resources."""

    def test_compact_groups_accept_strided_float32_views(self) -> None:
        """Copy a strided view once and retain its row count."""
        positions = np.zeros((20, 3), dtype=np.float32)
        cloud = PointCloud([(positions[::2], (10, 80, 120))])
        positions[:] = np.nan
        assert cloud.point_count == 10

    def test_composition_keeps_the_source_cloud_and_element_palette(self) -> None:
        """Molecular snapshots do not consume or mutate point groups."""
        cloud = PointCloud([(np.zeros((2, 3), dtype=np.float32), (0, 0, 0))])
        composed = cloud.with_scene(Scene())
        assert composed.point_count == cloud.point_count == 2
        assert color.element_rgb(8) == (216, 40, 40)

    def test_invalid_positions_and_empty_groups_are_rejected(self) -> None:
        """A malformed cloud cannot reach GPU packing."""
        with self.assertRaises(MolgfxError):
            PointCloud([])
        with self.assertRaises(ValueError):
            PointCloud([(np.zeros((2, 2), dtype=np.float32), (0, 0, 0))])
        with self.assertRaises(MolgfxError):
            PointCloud([(np.full((1, 3), np.nan, dtype=np.float32), (0, 0, 0))])
        with self.assertRaises(MolgfxError):
            PointCloud([(np.zeros((1, 3), dtype=np.float32), (0, 0, 0))], radius=0)

    def test_native_framing_contains_an_anisotropic_cloud(self) -> None:
        """The native policy fits the longest dimension into the wider viewport."""
        camera = Camera.frame_bounds((-20.0, -2.0, -1.0), (20.0, 2.0, 1.0), aspect=1.5)
        for position in ((-20.0, -2.0, -1.0), (20.0, 2.0, 1.0)):
            projected = camera.project(position, (1500, 1000))
            assert projected is not None
            assert 0 <= projected[0] <= 1500
            assert 0 <= projected[1] <= 1000
        with self.assertRaises(MolgfxError):
            Camera.frame_bounds((1.0, 0.0, 0.0), (0.0, 0.0, 0.0))

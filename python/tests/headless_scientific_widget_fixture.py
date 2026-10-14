"""Emit real scientific bindings through the canonical Python widget."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
from typing import TYPE_CHECKING, Literal

import molframe
import numpy as np
from headless_widget_fixture import _state
from test_hydrogen_bonds import PDB

import molgfx

if TYPE_CHECKING:
    from molgfx._engine import TrajectoryFrame


def _bind_motion(scene: molgfx.Scene, *, playback: bool) -> tuple[TrajectoryFrame, TrajectoryFrame]:
    """Keep original frame owners available for interval reversal."""
    start = molgfx.trajectory.frame(0, 0, ((0, 0, 0), (1, 0, 0), (2.8, 0, 0)))
    end = molgfx.trajectory.frame(1, 1, ((0, 4, 0), (1, 4, 0), (2.8, 4, 0)))
    if playback:
        scene.set_trajectory_pair(
            structure=scene.structure_id, start=start, end=end, sample_time=0.25
        )
    else:
        scene.add(
            molgfx.trajectory.bind(
                structure=scene.structure_id,
                source=molgfx.data.source("scientific-widget-motion"),
                frame_count=2,
            )
        )
        scene.bind_trajectory(
            source_hash="scientific-widget-motion", start=start, end=end, sample_time=0.25
        )
    return start, end


def _method() -> str:
    """Select one real scientific transport scenario."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "method",
        choices=(
            "volume",
            "hydrogen_bonds",
            "surf",
            "msms",
            "trajectory",
            "sampled_surf",
            "sampled_msms",
            "sampled_hydrogen_bonds",
            "candidate_hydrogen_bonds",
            "playback",
            "playback_hydrogen_bonds",
            "sampled_playback_hydrogen_bonds",
        ),
    )
    return str(parser.parse_args().method)


def _bind_surface(scene: molgfx.Scene, method: str, *, sampled: bool) -> None:
    """Use real provider geometry while retaining its exact calculation input."""
    actual_method: Literal["surf", "msms"] = "surf" if method.endswith("surf") else "msms"
    pose = scene.molecular_pose(structure=scene.structure_id) if sampled else None
    provider = molframe.surface.NativeSurfaceProvider(
        os.environ[f"MOLFRAME_{actual_method.upper()}"], actual_method, probe=1.4
    )
    with tempfile.TemporaryDirectory() as directory:
        output = molframe.surface.native_surface(
            np.array(
                [
                    [0, 1 if sampled else 0, 0],
                    [1, 1 if sampled else 0, 0],
                    [2.8, 1 if sampled else 0, 0],
                ],
                dtype=np.float32,
            ),
            np.array([1.55, 1.2, 1.52], dtype=np.float32),
            np.array([2], dtype=np.uint32),
            provider,
            context=molframe.ExecutionContext(temp_directory=directory, temp_bytes=1_000_000_000),
        )
        representation = molgfx.rep.surf if actual_method == "surf" else molgfx.rep.msms
        identity = scene.add(
            representation(target="all", source=molgfx.data.source("scientific-widget-surface"))
        )
        scene.bind_molecular_surface(identity, output, pose=pose)
    scene.set_camera(scene.frame("all"))


def main() -> None:
    """Acquire scientific data in the host and keep rendering in the shared viewer."""
    method = _method()
    scene = molgfx.Scene() if method == "volume" else molgfx.Scene(molframe.read(PDB, format="pdb"))
    sampled = method.startswith("sampled_")
    playback = "playback" in method
    moving = sampled or playback or method in {"trajectory", "candidate_hydrogen_bonds"}
    start, end = _bind_motion(scene, playback=playback) if moving else (None, None)
    if method in {"trajectory", "playback"}:
        scene.add(molgfx.rep.spacefill(target="all"))
        scene.set_camera(scene.frame("all"))
    elif method == "volume":
        volume = scene.add(
            molgfx.density.volume(
                source=molgfx.data.source("scientific-widget-grid"),
                dimensions=(5, 5, 5),
                voxel_to_world=(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -2, -2, -2, 1),
            ).isosurface(1.5, color=(30, 100, 230))
        )
        scene.bind_volume(
            volume,
            [
                float((x - 2) ** 2 + (y - 2) ** 2 + (z - 2) ** 2)
                for z in range(5)
                for y in range(5)
                for x in range(5)
            ],
        )
    elif method.endswith("hydrogen_bonds"):
        interaction = scene.add(
            molgfx.interaction.hydrogen_bonds(
                structure=scene.structure_id,
                source=molgfx.data.source("scientific-widget-hbonds"),
                geometry=(3.5, 150, 2.5),
                width=5,
                color=(220, 40, 40) if moving else (128, 204, 255),
            )
        )
        scene.bind_hydrogen_bonds(
            interaction, [(0, 1, 2)], candidate_distance=None if sampled else 3.5
        )
        scene.set_camera(scene.frame("all"))
    else:
        _bind_surface(scene, method, sampled=sampled)
    viewer = molgfx.Viewer(scene)
    try:
        initial = _state(viewer)
        samples: dict[str, object] = {}
        if moving:
            if playback:
                assert start is not None
                assert end is not None
                scene.set_trajectory_pair(
                    structure=scene.structure_id,
                    start=end,
                    end=molgfx.trajectory.frame(2, 2, ((0, 8, 0), (1, 8, 0), (2.8, 8, 0))),
                    sample_time=1.25,
                )
            else:
                scene.set_trajectory_time(structure=scene.structure_id, seconds=0.75)
            samples["advanced"] = _state(viewer)
            viewer.sync_request += 1
            samples["stale_resync"] = _state(viewer)
            if playback:
                assert start is not None
                assert end is not None
                scene.set_trajectory_pair(
                    structure=scene.structure_id, start=start, end=end, sample_time=0.25
                )
            else:
                scene.set_trajectory_time(structure=scene.structure_id, seconds=0.25)
            samples["reversed"] = _state(viewer)
        # A real authored camera patch must follow the restored semantic revision.
        scene.set_camera(None)
        patched = _state(viewer)
        viewer.sync_request += 1
        resynced = _state(viewer)
        print(json.dumps({"initial": initial, "patched": patched, "resynced": resynced, **samples}))
    finally:
        viewer.close()


if __name__ == "__main__":
    main()

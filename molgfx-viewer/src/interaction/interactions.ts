import {
  CAMERA_PUBLISH_DELAY_MS,
  DRAG_THRESHOLD_PX,
  KEY_ROTATION_STEP_PX,
  KEY_ZOOM_DELTA,
} from "../core/config.js";
import { CleanupBag, listen } from "../core/lifecycle.js";
import type { Camera, ExactAtom, MeasurementKind, MeasurementRequest } from "../core/types.js";
import { interactionModifiers } from "../core/interaction.js";
import type { InteractionModifiers } from "../core/interaction.js";
import type { RenderLoop } from "../render/render-loop.js";
import { pickCoordinates, rotateCamera, zoomCamera } from "../render/camera.js";
interface InteractionOptions {
  canvas: HTMLCanvasElement;
  loop: RenderLoop;
  publishCamera: (camera: Camera) => void;
  onPick: (x: number, y: number, modifiers: SelectionModifiers) => void;
  onClearSelection: () => void;
  /** A hover settled on something (or moved off everything): local-only, never published. */
  onHover: (result: string | undefined) => void;
}

export type SelectionModifiers = InteractionModifiers;
interface PointerState {
  id: number;
  startX: number;
  startY: number;
  x: number;
  y: number;
  dragged: boolean;
}

function wheelPixels(event: WheelEvent, canvas: HTMLCanvasElement): number {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
    return event.deltaY * 16;
  }

  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return event.deltaY * canvas.clientHeight;
  }

  return event.deltaY;
}

const KEY_ACTIONS: Record<string, (camera: Camera) => void> = {
  ArrowLeft: (camera) => rotateCamera(camera, KEY_ROTATION_STEP_PX, 0),
  ArrowRight: (camera) => rotateCamera(camera, -KEY_ROTATION_STEP_PX, 0),
  ArrowUp: (camera) => rotateCamera(camera, 0, -KEY_ROTATION_STEP_PX),
  ArrowDown: (camera) => rotateCamera(camera, 0, KEY_ROTATION_STEP_PX),
  "+": (camera) => zoomCamera(camera, -KEY_ZOOM_DELTA),
  "=": (camera) => zoomCamera(camera, -KEY_ZOOM_DELTA),
  "-": (camera) => zoomCamera(camera, KEY_ZOOM_DELTA),
};

export interface ViewerInteractions {
  cancelPendingPublish(): void;
  /** Release pointer capture and pending gesture state; idempotent. */
  cancelPendingGesture(): void;
  dispose(): void;
}

export function mountInteractions(options: InteractionOptions): ViewerInteractions {
  const { canvas, loop, publishCamera, onPick, onClearSelection, onHover } = options;
  const cleanup = new CleanupBag();

  let pointer: PointerState | undefined;
  let suppressClick = false;
  let publishTimer: ReturnType<typeof setTimeout> | undefined;

  let hoverToken = 0;
  let hoverRunning = false;
  let hoverNext: [number, number] | undefined;

  const cancelHover = () => {
    hoverNext = undefined;
    hoverToken += 1;
  };

  const cancelPendingGesture = (): void => {
    releasePointer();
    suppressClick = false;
  };

  const drainHover = async () => {
    hoverRunning = true;
    try {
      while (hoverNext) {
        const coordinates = hoverNext;
        hoverNext = undefined;
        const [x, y] = coordinates;
        const token = ++hoverToken;
        try {
          const { performed, result } = await loop.pick(x, y);
          if (performed && token === hoverToken) {
            onHover(result);
          }
        } catch {
          if (token === hoverToken) {
            onHover(undefined);
          }
        }
      }
    } finally {
      hoverRunning = false;
    }
  };

  const scheduleHover = (x: number, y: number) => {
    hoverNext = [x, y];
    if (!hoverRunning) {
      void drainHover();
    }
  };

  const cancelPublish = () => {
    if (publishTimer === undefined) {
      return;
    }
    clearTimeout(publishTimer);
    publishTimer = undefined;
  };

  const publishNow = () => {
    cancelPublish();
    publishCamera(loop.camera);
  };

  const publishSoon = () => {
    cancelPublish();
    publishTimer = setTimeout(publishNow, CAMERA_PUBLISH_DELAY_MS);
  };

  const updateCamera = (update: (camera: Camera) => void) => {
    update(loop.camera);
    loop.requestFrame();
    publishSoon();
  };

  cleanup.add(
    listen(canvas, "click", (event) => {
      if (suppressClick) {
        suppressClick = false;
        return;
      }

      const coordinates = pickCoordinates(canvas, event.clientX, event.clientY);
      if (coordinates) {
        onPick(...coordinates, interactionModifiers(event));
      }
    }),
  );

  cleanup.add(
    listen(canvas, "pointerdown", (event) => {
      if (!event.isPrimary || event.button !== 0 || pointer) {
        return;
      }

      pointer = {
        id: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        x: event.clientX,
        y: event.clientY,
        dragged: false,
      };

      suppressClick = false;
      cancelHover();
      onHover(undefined);
      canvas.setPointerCapture(event.pointerId);
    }),
  );

  cleanup.add(
    listen(canvas, "pointermove", (event) => {
      if (pointer && pointer.id === event.pointerId) {
        const dx = event.clientX - pointer.x;
        const dy = event.clientY - pointer.y;
        const totalX = event.clientX - pointer.startX;
        const totalY = event.clientY - pointer.startY;

        pointer.dragged ||= Math.hypot(totalX, totalY) >= DRAG_THRESHOLD_PX;

        rotateCamera(loop.camera, dx, dy);
        pointer.x = event.clientX;
        pointer.y = event.clientY;
        loop.requestFrame();
        return;
      }

      if (pointer) {
        return;
      }

      const coordinates = pickCoordinates(canvas, event.clientX, event.clientY);
      if (coordinates) {
        scheduleHover(...coordinates);
      }
    }),
  );

  cleanup.add(
    listen(canvas, "pointerleave", () => {
      cancelHover();
      onHover(undefined);
    }),
  );

  const releasePointer = (): void => {
    if (pointer === undefined) {
      return;
    }
    const { id } = pointer;
    pointer = undefined;
    if (canvas.hasPointerCapture(id)) {
      canvas.releasePointerCapture(id);
    }
  };

  const finishPointer = (event: PointerEvent, suppressNextClick: boolean) => {
    if (!pointer || pointer.id !== event.pointerId) {
      return;
    }

    suppressClick = suppressNextClick && pointer.dragged;
    releasePointer();

    publishNow();
  };

  cleanup.add(listen(canvas, "pointerup", (event) => finishPointer(event, true)));
  cleanup.add(listen(canvas, "pointercancel", (event) => finishPointer(event, false)));

  cleanup.add(
    listen(
      canvas,
      "wheel",
      (event) => {
        event.preventDefault();
        updateCamera((camera) => zoomCamera(camera, wheelPixels(event, canvas)));
      },
      { passive: false },
    ),
  );

  cleanup.add(
    listen(canvas, "keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        pointer = undefined;
        suppressClick = true;
        cancelPublish();
        cancelHover();
        onHover(undefined);
        onClearSelection();
        return;
      }

      const action = KEY_ACTIONS[event.key];
      if (!action) {
        return;
      }

      event.preventDefault();
      updateCamera(action);
    }),
  );


  return {
    cancelPendingPublish: cancelPublish,
    cancelPendingGesture,
    dispose() {
      cancelPublish();
      cancelHover();
      cancelPendingGesture();
      cleanup.dispose();
    },
  };
}
export class MeasurementGesture {
  #atoms: ExactAtom[] = [];
  #topologyRevision: bigint | undefined;

  add(atom: ExactAtom): boolean {
    if (this.#topologyRevision === undefined) this.#topologyRevision = atom.topologyRevision;
    if (atom.topologyRevision !== this.#topologyRevision) { this.cancel(); return false; }
    if (this.#atoms.some((existing) => existing.structure === atom.structure && existing.atom === atom.atom)) return false;
    this.#atoms.push(atom);
    return true;
  }

  cancel(): void { this.#atoms = []; this.#topologyRevision = undefined; }

  stale(topologyRevision: bigint): boolean {
    return this.#topologyRevision !== undefined && this.#topologyRevision !== topologyRevision;
  }

  take(kind: MeasurementKind): MeasurementRequest | undefined {
    const arity = kind === "distance" ? 2 : kind === "angle" ? 3 : 4;
    if (this.#atoms.length !== arity || this.#topologyRevision === undefined) return undefined;
    const request = { kind, atoms: this.#atoms as readonly ExactAtom[], topologyRevision: this.#topologyRevision };
    this.cancel();
    return request;
  }
}

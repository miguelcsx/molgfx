import { CleanupBag, listen } from "../core/lifecycle.js";
import { interactionModifiers } from "../core/interaction.js";
import { cameraState, pickCoordinates } from "../render/camera.js";
import type { RuntimeModule } from "../core/types.js";
import type { InteractionModifiers } from "../core/interaction.js";
import type { CameraState } from "../types.js";
import type { RenderLoop } from "../render/render-loop.js";
interface InteractionOptions {
  canvas: HTMLCanvasElement;
  loop: RenderLoop;
  runtime: RuntimeModule;
  publishCamera(camera: CameraState): void;
  onPick(x: number, y: number, modifiers: InteractionModifiers): void;
  onClearSelection(): void;
  onError(error: unknown): void;
}
export interface ViewerInteractions {
  cancelPendingGesture(): void;
  cancelPendingPublish(): void;
  dispose(): void;
}
export function mountInteractions(
  options: InteractionOptions,
): ViewerInteractions {
  const { canvas, loop, runtime } = options;
  const cleanup = new CleanupBag();
  const controller = new runtime.OrbitController();
  let pointer:
    | {
        id: number;
        x: number;
        y: number;
        startX: number;
        startY: number;
        button: string;
        dragged: boolean;
      }
    | undefined;
  let suppressClick = false,
    hoverToken = 0;
  let timer: number | undefined;
  const cancelPublish = () => {
    clearTimeout(timer);
    timer = undefined;
  };
  const publish = () => {
    cancelPublish();
    if (!loop.disposed) options.publishCamera(cameraState(loop.camera));
  };
  const perform = (operation: () => void) => {
    if (!loop.hasScene || loop.disposed) return;
    try {
      operation();
    } catch (error) {
      options.onError(error);
    }
  };
  const cancelGesture = () => {
    const held = pointer;
    pointer = undefined;
    hoverToken += 1;
    if (!held) return;
    if (!loop.disposed)
      loop.setCamera(
        controller.pointerButton(
          held.button,
          false,
          held.x,
          held.y,
          loop.camera,
        ),
      );
    if (canvas.hasPointerCapture(held.id))
      canvas.releasePointerCapture(held.id);
  };
  cleanup.add(listen(canvas, "contextmenu", (event) => event.preventDefault()));
  cleanup.add(
    listen(canvas, "pointerdown", (event) =>
      perform(() => {
        if (!event.isPrimary || pointer || event.button > 2) return;
        const button = ["left", "middle", "right"][event.button]!;
        pointer = {
          id: event.pointerId,
          x: event.clientX,
          y: event.clientY,
          startX: event.clientX,
          startY: event.clientY,
          button,
          dragged: false,
        };
        suppressClick = false;
        hoverToken += 1;
        canvas.focus();
        loop.setCamera(
          controller.pointerButton(
            button,
            true,
            event.clientX,
            event.clientY,
            loop.camera,
          ),
        );
        canvas.setPointerCapture(event.pointerId);
      }),
    ),
  );
  cleanup.add(
    listen(canvas, "pointermove", (event) =>
      perform(() => {
        if (pointer?.id === event.pointerId) {
          pointer.x = event.clientX;
          pointer.y = event.clientY;
          pointer.dragged ||=
            Math.hypot(
              event.clientX - pointer.startX,
              event.clientY - pointer.startY,
            ) >= 3;
          loop.setCamera(
            controller.pointerMove(event.clientX, event.clientY, loop.camera),
          );
          return;
        }
        if (pointer) return;
        const point = pickCoordinates(canvas, event.clientX, event.clientY);
        if (!point) return;
        const token = ++hoverToken;
        void loop.pickHover(...point).then(
          ({ performed, result }) => {
            if (token === hoverToken && performed && !loop.disposed)
              canvas.classList.toggle(
                "is-hovering-entity",
                result !== undefined,
              );
          },
          (error) => {
            if (!loop.disposed) options.onError(error);
          },
        );
      }),
    ),
  );
  const finish = (event: PointerEvent) =>
    perform(() => {
      if (event.pointerId !== pointer?.id) return;
      suppressClick = pointer.dragged || pointer.button !== "left";
      cancelGesture();
      publish();
    });
  cleanup.add(listen(canvas, "pointerup", finish));
  cleanup.add(listen(canvas, "pointercancel", finish));
  cleanup.add(
    listen(canvas, "lostpointercapture", () => perform(cancelGesture)),
  );
  cleanup.add(
    listen(canvas, "pointerleave", () => {
      hoverToken += 1;
      canvas.classList.remove("is-hovering-entity");
    }),
  );
  cleanup.add(
    listen(canvas, "click", (event) =>
      perform(() => {
        if (suppressClick) {
          suppressClick = false;
          return;
        }
        const point = pickCoordinates(canvas, event.clientX, event.clientY);
        if (point) options.onPick(...point, interactionModifiers(event));
      }),
    ),
  );
  cleanup.add(
    listen(
      canvas,
      "wheel",
      (event) =>
        perform(() => {
          event.preventDefault();
          const pixels =
            event.deltaY *
            (event.deltaMode === 1
              ? 16
              : event.deltaMode === 2
                ? canvas.clientHeight
                : 1);
          loop.setCamera(controller.scroll(-pixels / 120, loop.camera));
          cancelPublish();
          timer = setTimeout(publish, 200);
        }),
      { passive: false },
    ),
  );
  cleanup.add(
    listen(canvas, "keydown", (event) =>
      perform(() => {
        if (event.key === "Escape") {
          event.preventDefault();
          cancelGesture();
          cancelPublish();
          options.onClearSelection();
          return;
        }
        if (["+", "=", "-"].includes(event.key)) {
          event.preventDefault();
          loop.setCamera(
            controller.scroll(event.key === "-" ? -1 : 1, loop.camera),
          );
          publish();
          return;
        }
        const moves: Record<string, [number, number]> = {
          ArrowLeft: [-14, 0],
          ArrowRight: [14, 0],
          ArrowUp: [0, -14],
          ArrowDown: [0, 14],
        };
        const move = moves[event.key];
        if (!move) return;
        event.preventDefault();
        loop.setCamera(
          controller.pointerButton("left", true, 0, 0, loop.camera),
        );
        loop.setCamera(controller.pointerMove(...move, loop.camera));
        loop.setCamera(
          controller.pointerButton("left", false, ...move, loop.camera),
        );
        publish();
      }),
    ),
  );
  return {
    cancelPendingGesture: cancelGesture,
    cancelPendingPublish: cancelPublish,
    dispose() {
      cancelPublish();
      cancelGesture();
      cleanup.dispose();
      controller.free();
    },
  };
}

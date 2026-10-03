import type { CameraState } from "../types.js";
import type { WasmCamera } from "../core/types.js";
const PIXEL_BUDGET = 2560 * 1600;
export function dimensions(canvas: HTMLCanvasElement): [number, number] {
  const width = Math.max(1, canvas.clientWidth),
    height = Math.max(1, canvas.clientHeight);
  const ratio = Math.min(
    window.devicePixelRatio || 1,
    Math.sqrt(PIXEL_BUDGET / (width * height)),
  );
  return [
    Math.max(1, Math.round(width * ratio)),
    Math.max(1, Math.round(height * ratio)),
  ];
}
export function cameraState(camera: WasmCamera): CameraState {
  const position = camera.position,
    target = camera.target,
    up = camera.up;
  return {
    position: [position[0]!, position[1]!, position[2]!],
    target: [target[0]!, target[1]!, target[2]!],
    up: [up[0]!, up[1]!, up[2]!],
  };
}
export function pickCoordinates(
  canvas: HTMLCanvasElement,
  clientX: number,
  clientY: number,
): [number, number] | undefined {
  const bounds = canvas.getBoundingClientRect();
  if (bounds.width <= 0 || bounds.height <= 0) return undefined;
  const x = Math.floor(((clientX - bounds.left) * canvas.width) / bounds.width);
  const y = Math.floor(
    ((clientY - bounds.top) * canvas.height) / bounds.height,
  );
  if (x < 0 || x >= canvas.width || y < 0 || y >= canvas.height)
    return undefined;
  return [x, y];
}

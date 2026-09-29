import {
  CAMERA_PITCH_LIMIT,
  CAMERA_ROTATION_SPEED,
  CAMERA_ZOOM_LIMIT,
  CAMERA_ZOOM_SPEED,
  PIXEL_BUDGET,
} from "../core/config.js";
import type { Camera, WasmScene } from "../core/types.js";

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function isVec3(value: unknown): value is [number, number, number] {
  return (
    Array.isArray(value) && value.length === 3 && value.every((component) => Number.isFinite(component))
  );
}

function isCamera(value: unknown): value is Camera {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<Camera>;
  return isVec3(candidate.position) && isVec3(candidate.target) && isVec3(candidate.up);
}

export function dimensions(canvas: HTMLCanvasElement): [number, number] {
  const cssWidth = Math.max(1, canvas.clientWidth);
  const cssHeight = Math.max(1, canvas.clientHeight);

  const ratio = Math.min(
    window.devicePixelRatio || 1,
    Math.sqrt(PIXEL_BUDGET / (cssWidth * cssHeight)),
  );

  return [Math.max(1, Math.round(cssWidth * ratio)), Math.max(1, Math.round(cssHeight * ratio))];
}

export function sceneCamera(scene: WasmScene, width: number, height: number): Camera {
  const parsed: unknown = JSON.parse(scene.cameraJSON(width, height));

  if (!isCamera(parsed)) {
    throw new Error("runtime returned an invalid camera");
  }

  return parsed;
}

export function rotateCamera(camera: Camera, dx: number, dy: number): void {
  const [px, py, pz] = camera.position;
  const [tx, ty, tz] = camera.target;

  const ox = px - tx;
  const oy = py - ty;
  const oz = pz - tz;

  const radius = Math.hypot(ox, oy, oz);
  if (radius === 0) {
    return;
  }

  const yaw = Math.atan2(ox, oz) - dx * CAMERA_ROTATION_SPEED;
  const pitch = clamp(
    Math.asin(clamp(oy / radius, -1, 1)) + dy * CAMERA_ROTATION_SPEED,
    -CAMERA_PITCH_LIMIT,
    CAMERA_PITCH_LIMIT,
  );

  const horizontal = radius * Math.cos(pitch);

  camera.position[0] = tx + horizontal * Math.sin(yaw);
  camera.position[1] = ty + radius * Math.sin(pitch);
  camera.position[2] = tz + horizontal * Math.cos(yaw);
}

export function zoomCamera(camera: Camera, delta: number): void {
  const scale = Math.exp(clamp(delta * CAMERA_ZOOM_SPEED, -CAMERA_ZOOM_LIMIT, CAMERA_ZOOM_LIMIT));
  const [tx, ty, tz] = camera.target;

  camera.position[0] = tx + (camera.position[0] - tx) * scale;
  camera.position[1] = ty + (camera.position[1] - ty) * scale;
  camera.position[2] = tz + (camera.position[2] - tz) * scale;
}

export function pickCoordinates(
  canvas: HTMLCanvasElement,
  clientX: number,
  clientY: number,
): [number, number] | null {
  const bounds = canvas.getBoundingClientRect();
  if (bounds.width <= 0 || bounds.height <= 0) {
    return null;
  }

  const x = Math.floor(((clientX - bounds.left) * canvas.width) / bounds.width);
  const y = Math.floor(((clientY - bounds.top) * canvas.height) / bounds.height);

  return [clamp(x, 0, canvas.width - 1), clamp(y, 0, canvas.height - 1)];
}

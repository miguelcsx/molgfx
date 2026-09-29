// wasm-pack writes these bindings into pkg/ (git-ignored); see scripts/build.mjs.
import type { Renderer, Scene } from "../../pkg/molgfx_wasm.js";

export type Cleanup = () => void;

export type RuntimeModule = typeof import("../../pkg/molgfx_wasm.js");

export type WasmRenderer = Renderer;
export type WasmScene = Scene;

export type Vec3 = [number, number, number];

export interface Camera {
  position: Vec3;
  target: Vec3;
  up: Vec3;
  /** Distance the scene framed the camera at; limits how far zoom may travel. */
  distance?: number;
}

export type BinaryState = ArrayBuffer | ArrayBufferView;

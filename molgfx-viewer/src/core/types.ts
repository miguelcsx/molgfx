// wasm-pack writes these bindings into wasm/ (git-ignored); see wasm/README.
import type { Renderer, Scene } from "../../wasm/molgfx_wasm.js";

export type Cleanup = () => void;

export type RuntimeModule = typeof import("../../wasm/molgfx_wasm.js");

export type WasmRenderer = Renderer;
export type WasmScene = Scene;

export type Vec3 = [number, number, number];

export interface Camera {
  position: Vec3;
  target: Vec3;
  up: Vec3;
}

export type BinaryState = ArrayBuffer | ArrayBufferView;

/** Stable identity captured by a measurement gesture before readback. */
export interface ExactAtom {
  readonly structure: bigint;
  readonly atom: number;
  readonly topologyRevision: bigint;
}

export type MeasurementKind = "distance" | "angle" | "dihedral";

/** A retained measurement request, kept independent of renderer resources. */
export interface MeasurementRequest {
  readonly kind: MeasurementKind;
  readonly atoms: readonly ExactAtom[];
  readonly topologyRevision: bigint;
}

export interface VolumeSigmaControls {
  readonly positiveSigma?: number;
  readonly negativeSigma?: number;
}

export interface TrajectoryFrameInput {
  readonly structure: bigint;
  readonly topologyRevision: bigint;
  readonly frame: number;
  readonly time: number;
  readonly positions: BinaryState;
}

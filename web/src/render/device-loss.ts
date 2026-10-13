/** The slice of a renderer the recovery path needs; keeps the wasm ABI out of public types. */
export interface LossAwareRenderer {
  deviceLossJSON(): string | undefined | null;
  free(): void;
}

/** Why the GPU device was lost, as the renderer classified it. */
export interface DeviceLoss {
  reason: "destroyed" | "unknown";
  /** Whether opening a fresh renderer on the same scene is expected to work. */
  recoverable: boolean;
  message: string;
}

/** The loss as seen by the viewer, with the outcome of the recreate attempt. */
export interface DeviceLostEvent extends DeviceLoss {
  /** True when a replacement renderer is already drawing the scene. */
  recreated: boolean;
}

/** The typed loss recorded by `renderer`, or `undefined` while it is healthy. */
export function readDeviceLoss(
  renderer: Pick<LossAwareRenderer, "deviceLossJSON">,
): DeviceLoss | undefined {
  const source = renderer.deviceLossJSON();
  if (source === undefined || source === null) return undefined;
  const value = JSON.parse(source) as Partial<DeviceLoss>;
  return {
    reason: value.reason === "destroyed" ? "destroyed" : "unknown",
    recoverable: value.recoverable === true,
    message: typeof value.message === "string" ? value.message : "",
  };
}

interface RecoveryLoop<R> {
  replaceRenderer(next: R): void;
  settled(): Promise<void>;
}

/**
 * Replaces a lost renderer with a fresh one that rebuilds every GPU resource
 * from the scene on its first frame. A loss that is not recoverable keeps the
 * old renderer untouched. Resolves with the renderer now in use.
 */
export async function recoverRenderer<R extends LossAwareRenderer>(options: {
  loss: DeviceLoss;
  current: R;
  loop: RecoveryLoop<R>;
  create: () => Promise<R>;
  isDisposed: () => boolean;
}): Promise<{ renderer: R; recreated: boolean }> {
  const { loss, current, loop, create, isDisposed } = options;
  if (!loss.recoverable) return { renderer: current, recreated: false };
  const next = await create();
  if (isDisposed()) {
    next.free();
    return { renderer: current, recreated: false };
  }
  loop.replaceRenderer(next);
  // A readback begun on the lost device must finish before its owner is freed.
  await loop.settled();
  current.free();
  return { renderer: next, recreated: true };
}

import type {
  SequenceFrame,
  SequenceManifest,
  SequenceManifestFrame,
  SequenceOptions,
  TimeChange,
  TransportEvents,
} from "../core/contracts.js";

/** What the sequence needs from the transport; `Transport` satisfies it. */
export interface SequencePlayback {
  readonly durationSeconds: number;
  pause(): void;
  seek(seconds: number): void;
  on<K extends keyof TransportEvents>(
    type: K,
    listener: (event: TransportEvents[K]) => void,
  ): () => void;
}

/** What the sequence needs from the canvas and renderer, with no animation-frame scheduling. */
export interface SequenceSurface {
  /** Fixes the output size and suspends on-demand drawing. */
  begin(width: number, height: number): void;
  /** Renders the current scene once, synchronously; false when the pass cap was hit before convergence. */
  settle(): boolean;
  /** The error the scene apply raised since the last call, cleared on read. */
  takeError(): unknown;
  /** Starts encoding the canvas as it stands now. */
  encode(): Promise<Blob>;
  /** Restores the canvas size and on-demand drawing. */
  end(): void;
}

export const INTERPOLATION_NOTE =
  "Times come from file timestamps. Between saved frames the motion is linear interpolation.";

function positiveInteger(value: number, name: string): void {
  if (!Number.isInteger(value) || value <= 0)
    throw new RangeError(`${name} must be a positive integer`);
}

function captureError(index: number, cause: unknown): Error {
  const detail = cause instanceof Error ? cause.message : String(cause);
  return new Error(`Sequence capture failed at frame ${index}: ${detail}`, { cause });
}

/**
 * Captures `i / fps` for each frame by stepping the transport by hand: seek,
 * settle, encode, deliver, in that order and one frame at a time. A scene
 * apply error stops the capture: it throws an error naming the frame index and
 * no later frame is delivered.
 */
export async function captureFrameSequence(
  transport: SequencePlayback,
  surface: SequenceSurface,
  options: SequenceOptions,
): Promise<SequenceManifest> {
  const { fps, size, onFrame, signal } = options;
  positiveInteger(fps, "fps");
  positiveInteger(size.width, "size.width");
  positiveInteger(size.height, "size.height");
  const seconds = options.seconds ?? transport.durationSeconds;
  if (!(seconds > 0) || seconds > transport.durationSeconds)
    throw new RangeError("seconds must be positive and within the transport duration");
  const total = Math.floor(seconds * fps + 1e-9);
  if (total < 1) throw new RangeError("the range holds no frame at this fps");
  const manifest: SequenceManifest = {
    fps,
    frameCount: 0,
    completed: false,
    unconvergedFrames: 0,
    width: size.width,
    height: size.height,
    interpolation: INTERPOLATION_NOTE,
    frames: [],
  };
  let change: TimeChange | undefined;
  const stop = transport.on("timechange", (event) => {
    change = event;
  });
  transport.pause();
  surface.begin(size.width, size.height);
  const frame: SequenceFrame = {
    index: 0,
    playbackSeconds: 0,
    image: undefined as never,
    readouts: [],
  };
  try {
    for (let index = 0; index < total; index += 1) {
      if (signal?.aborted) return manifest;
      const playbackSeconds = index / fps;
      change = undefined;
      transport.seek(playbackSeconds);
      const failure = surface.takeError();
      if (failure !== undefined) throw captureError(index, failure);
      const converged = surface.settle();
      // Encoding is requested in the same task as the render so the canvas
      // still holds this frame; awaiting it keeps one frame in flight.
      const image = await surface.encode();
      const readouts = change === undefined ? frame.readouts : (change as TimeChange).readouts;
      const tracks: SequenceManifestFrame["tracks"] = [];
      for (const readout of readouts)
        tracks.push({ track: readout.track, simSeconds: readout.seconds, unit: readout.unit });
      manifest.frames.push({ index, playbackSeconds, converged, tracks });
      if (!converged) manifest.unconvergedFrames += 1;
      manifest.frameCount = index + 1;
      frame.index = index;
      frame.playbackSeconds = playbackSeconds;
      frame.image = image;
      frame.readouts = readouts;
      await onFrame(frame);
    }
    manifest.completed = true;
    return manifest;
  } finally {
    stop();
    transport.pause();
    surface.end();
  }
}

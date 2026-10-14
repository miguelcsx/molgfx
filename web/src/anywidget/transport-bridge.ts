import type {
  TimeChange,
  TransportLoopMode,
  TransportPort,
} from "../core/contracts.js";
import type { Cleanup } from "../core/transport.js";
import { observeMessages } from "./model.js";
import type { WidgetModel } from "./model.js";

const LOOP_MODES: ReadonlySet<string> = new Set<TransportLoopMode>(["clamp", "loop", "pingpong"]);

/** One track occupies this many slots of the `timechange` readouts: track, value, unit, phase. */
export const READOUT_SLOTS = 4;

/** Message sent to the kernel on every tick; the object and its array are reused. */
interface TimeChangeMessage {
  type: "timechange";
  seconds: number;
  /** Flat `[track, value, unit, phase, ...]` per track. */
  readouts: Array<number | string>;
}

function finite(content: Record<string, unknown>, key: string): number {
  const value = content[key];
  if (typeof value !== "number" || !Number.isFinite(value))
    throw new RangeError(`${String(content["type"])} requires a finite ${key}`);
  return value;
}

function forward(transport: TransportPort, content: unknown): void {
  if (typeof content !== "object" || content === null) return;
  const message = content as Record<string, unknown>;
  switch (message["type"]) {
    case "play":
      transport.play();
      return;
    case "pause":
      transport.pause();
      return;
    case "seek":
      transport.seek(finite(message, "seconds"));
      return;
    case "set_rate":
      transport.rate = finite(message, "rate");
      return;
    case "set_loop": {
      const mode = message["mode"];
      if (typeof mode !== "string" || !LOOP_MODES.has(mode))
        throw new RangeError("set_loop requires clamp, loop or pingpong");
      transport.loopMode = mode as TransportLoopMode;
      return;
    }
    default:
      // Other kernel messages (chunk replies travel as traits) are not transport commands.
      return;
  }
}

/**
 * Forwards kernel playback messages to one transport and reports its time
 * back. The outgoing message is mutated in place, so a tick adds no
 * allocation here beyond what the comm layer itself serializes.
 */
export function bindTransport(
  model: WidgetModel,
  transport: TransportPort,
  onError: (error: unknown) => void,
): Cleanup {
  const message: TimeChangeMessage = { type: "timechange", seconds: 0, readouts: [] };
  const stopMessages = observeMessages(model, (content) => {
    try {
      forward(transport, content);
    } catch (error) {
      onError(error);
    }
  });
  const stopTime = transport.on("timechange", (event: TimeChange) => {
    const { readouts } = event;
    message.seconds = event.playbackSeconds;
    message.readouts.length = readouts.length * READOUT_SLOTS;
    for (let index = 0; index < readouts.length; index += 1) {
      const readout = readouts[index];
      if (!readout) continue;
      const base = index * READOUT_SLOTS;
      message.readouts[base] = readout.track;
      message.readouts[base + 1] = readout.value;
      message.readouts[base + 2] = readout.unit;
      message.readouts[base + 3] = readout.phase;
    }
    model.send(message);
  });
  return () => {
    stopMessages();
    stopTime();
  };
}

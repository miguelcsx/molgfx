import type {
  TimelineLayout,
  TrackReadout,
  TransportTimeUnit,
} from "../core/contracts.js";
/** Seconds per display unit; the one place the unit symbols are named in TypeScript. */
const UNIT_SECONDS: Readonly<Record<TransportTimeUnit, number>> = {
  as: 1e-18,
  fs: 1e-15,
  ps: 1e-12,
  ns: 1e-9,
  us: 1e-6,
  ms: 1e-3,
  s: 1,
};
function isUnit(symbol: string): symbol is TransportTimeUnit {
  return Object.hasOwn(UNIT_SECONDS, symbol);
}
/** Validates the runtime's symbol table against the units this host can display. */
export function unitTable(layout: TimelineLayout): TransportTimeUnit[] {
  return layout.unitSymbols.map((symbol) => {
    if (!isUnit(symbol)) throw new Error(`Unknown time unit symbol: ${symbol}`);
    return symbol;
  });
}
/**
 * Decodes stride-sized rows into a fixed set of readout objects without
 * allocating: the buffer and the objects are created once per track count.
 */
export class ReadoutBuffer {
  readonly #stride: number;
  readonly #units: TransportTimeUnit[];
  readonly raw: Float64Array;
  readonly readouts: TrackReadout[];
  constructor(layout: TimelineLayout, trackCount: number) {
    this.#stride = layout.readingStride;
    this.#units = unitTable(layout);
    this.raw = new Float64Array(trackCount * this.#stride);
    this.readouts = Array.from({ length: trackCount }, () => ({
      track: 0,
      seconds: 0,
      phase: 0,
      unit: "s" as TransportTimeUnit,
      value: 0,
    }));
  }
  /** Decodes the first `written` rows of `raw` into `readouts`. */
  decode(written: number): void {
    const count = Math.min(written, this.readouts.length);
    for (let row = 0; row < count; row += 1) {
      const base = row * this.#stride;
      const out = this.readouts[row];
      const unit = this.#units[this.raw[base + 3] ?? -1];
      if (!out || !unit) throw new Error("Malformed timeline reading");
      out.track = this.raw[base] ?? 0;
      out.seconds = this.raw[base + 1] ?? 0;
      out.phase = this.raw[base + 2] ?? 0;
      out.unit = unit;
      out.value = out.seconds / UNIT_SECONDS[unit];
    }
  }
}

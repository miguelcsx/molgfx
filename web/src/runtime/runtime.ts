import type { InlineRuntimeSource } from "../core/contracts.js";
import type { RuntimeModule } from "../core/types.js";

const inlineByKey = new Map<string, Promise<RuntimeModule>>();
const inlineBySource = new WeakMap<
  InlineRuntimeSource,
  Promise<RuntimeModule>
>();
let staticRuntime: Promise<RuntimeModule> | undefined;

interface PromiseCache<K> {
  get(key: K): Promise<RuntimeModule> | undefined;
  set(key: K, value: Promise<RuntimeModule>): unknown;
  delete(key: K): boolean;
}

function cached<K>(
  cache: PromiseCache<K>,
  key: K,
  factory: () => Promise<RuntimeModule>,
): Promise<RuntimeModule> {
  const existing = cache.get(key);
  if (existing) {
    return existing;
  }

  const pending = factory().catch((error) => {
    if (cache.get(key) === pending) {
      cache.delete(key);
    }
    throw error;
  });

  cache.set(key, pending);
  return pending;
}

export function sourceBytes(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }

  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }

  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }

  throw new Error(
    "expected binary widget state, received " +
      Object.prototype.toString.call(value),
  );
}

async function inflate(bytes: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([bytes as BlobPart])
    .stream()
    .pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function instantiateStaticRuntime(): Promise<RuntimeModule> {
  const runtime = await import("../../generated/molgfx_wasm.js");
  const wasm = new URL("../../generated/molgfx_wasm_bg.wasm", import.meta.url);
  await runtime.default({ module_or_path: wasm });
  return runtime;
}

async function instantiateInlineRuntime(
  glue: string,
  source: InlineRuntimeSource,
): Promise<RuntimeModule> {
  const compressedWasm = source.wasmGzip();
  if (!compressedWasm) {
    throw new Error(
      "inline runtime source declared glue code but no wasm payload",
    );
  }

  const url = URL.createObjectURL(
    new Blob([glue], { type: "text/javascript" }),
  );

  try {
    const runtime = (await import(url)) as RuntimeModule;
    const wasm = await inflate(sourceBytes(compressedWasm));
    await runtime.default({ module_or_path: wasm });
    return runtime;
  } finally {
    URL.revokeObjectURL(url);
  }
}

/**
 * A notebook frontend loads this module from a blob URL, where a runtime
 * beside it cannot be imported by a relative path, so a kernel that packaged
 * one sends it inline through `inline`; a host with no such channel -- or a
 * checkout without a built runtime -- falls back to the static import beside
 * the bundle.
 */
export function loadRuntime(
  inline?: InlineRuntimeSource,
): Promise<RuntimeModule> {
  const glue = inline?.glue();

  if (!inline || !glue) {
    staticRuntime ??= instantiateStaticRuntime().catch((error) => {
      staticRuntime = undefined;
      throw error;
    });
    return staticRuntime;
  }

  const key = inline.key();
  return key
    ? cached(inlineByKey, key, () => instantiateInlineRuntime(glue, inline))
    : cached(inlineBySource, inline, () =>
        instantiateInlineRuntime(glue, inline),
      );
}

export function assertWebGPUAvailable(): void {
  if (typeof navigator !== "undefined" && "gpu" in navigator) {
    return;
  }

  throw new Error(
    "MolGFX needs a browser with WebGPU in a secure context (HTTPS or localhost).",
  );
}

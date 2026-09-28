// Assembles the Python package's static directory: the bundled widget, its
// stylesheet, and (when present) the wasm-pack runtime built into ../wasm.
// wasm/ is git-ignored and populated by:
//   wasm-pack build crates/molgfx-wasm --target web --out-dir ../molgfx-viewer/wasm --out-name molgfx_wasm
import { build } from "esbuild";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const outDir = join(root, "..", "python", "molgfx", "viewer", "static");
const wasmDir = join(root, "wasm");

mkdirSync(outDir, { recursive: true });

await build({
  entryPoints: [join(root, "src", "anywidget", "index.ts")],
  outfile: join(outDir, "widget.js"),
  bundle: true,
  format: "esm",
  platform: "browser",
  target: ["es2022"],
  minify: true,
  sourcemap: true,
  legalComments: "none",
  charset: "utf8",
  banner: { js: "// Generated from molgfx-viewer/src/anywidget/index.ts. Do not edit directly." },
});

copyFileSync(join(root, "src", "widget.css"), join(outDir, "widget.css"));

const runtimeFiles = ["molgfx_wasm.js", "molgfx_wasm_bg.wasm", "molgfx_wasm.d.ts"];
const missing = runtimeFiles.filter((name) => !existsSync(join(wasmDir, name)));

if (missing.length === 0) {
  for (const name of runtimeFiles) {
    copyFileSync(join(wasmDir, name), join(outDir, name));
  }
  const glue = readFileSync(join(wasmDir, "molgfx_wasm.js"));
  const wasm = readFileSync(join(wasmDir, "molgfx_wasm_bg.wasm"));
  const contentKey = createHash("sha256")
    .update(glue)
    .update(Buffer.from([0]))
    .update(wasm)
    .digest("hex")
    .slice(0, 16);
  writeFileSync(
    join(outDir, "runtime-manifest.json"),
    JSON.stringify({
      available: true,
      glue: "molgfx_wasm.js",
      wasm: "molgfx_wasm_bg.wasm",
      content_key: contentKey,
    }) + "\n",
  );
} else {
  writeFileSync(join(outDir, "runtime-manifest.json"), "{\"available\":false}\n");
  console.warn(
    "molgfx-viewer: no wasm-pack runtime in " + wasmDir +
      " (missing " + missing.join(", ") + "); " +
      "the widget will ship without a bundled browser runtime.",
  );
}

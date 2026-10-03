import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { assertVersion, web } from "./version.mjs";

export function checkArtifacts() {
  const version = assertVersion();
  const dist = join(web, "dist");
  const manifest = JSON.parse(
    readFileSync(join(dist, "runtime-manifest.json"), "utf8"),
  );
  if (!manifest.available || manifest.version !== version)
    throw new Error("Stale or unavailable runtime manifest");
  for (const [name, hash] of Object.entries(manifest.sha256)) {
    if (name.includes("..") || name.startsWith("/"))
      throw new Error("Unsafe manifest path");
    const actual = createHash("sha256")
      .update(readFileSync(join(dist, name)))
      .digest("hex");
    if (actual !== hash) throw new Error("Artifact hash mismatch: " + name);
  }
  const key = createHash("sha256")
    .update(readFileSync(join(dist, manifest.glue)))
    .update(Buffer.from([0]))
    .update(readFileSync(join(dist, manifest.wasm)))
    .digest("hex")
    .slice(0, 16);
  if (key !== manifest.content_key)
    throw new Error("Runtime content key mismatch");
  const pkg = JSON.parse(readFileSync(join(web, "package.json"), "utf8"));
  const visited = new Set();
  function visit(path) {
    if (visited.has(path)) return;
    visited.add(path);
    const source = readFileSync(path, "utf8");
    if (
      /generated|molgfx_wasm|InitOutput|WebAssembly\.Memory|__wbg_|wasm_bindgen/.test(
        source,
      )
    )
      throw new Error("Public declarations expose the wasm ABI: " + path);
    for (const match of source.matchAll(
      /(?:from\s*|import\s*\()(["'])([^"']+)\1/g,
    )) {
      const target = match[2];
      if (!target.startsWith(".")) continue;
      const candidate = resolve(
        dirname(path),
        target.replace(/\.js$/, ".d.ts"),
      );
      if (existsSync(candidate)) visit(candidate);
      else if (existsSync(candidate + ".d.ts")) visit(candidate + ".d.ts");
      else throw new Error("Missing public declaration dependency: " + target);
    }
  }
  for (const entry of Object.values(pkg.exports)) {
    if (typeof entry === "string") {
      if (!existsSync(join(web, entry)))
        throw new Error("Missing package export: " + entry);
      continue;
    }
    if (!existsSync(join(web, entry.import)))
      throw new Error("Missing package export: " + entry.import);
    visit(join(web, entry.types));
  }
  return manifest;
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  console.error("Verified MolGFX artifacts: " + checkArtifacts().content_key);
}

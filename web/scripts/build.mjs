// Build once; notebook and documentation consumers receive byte-identical assets.
import { build } from "esbuild";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { assertVersion, repo, web } from "./version.mjs";
import { checkArtifacts } from "./check-artifacts.mjs";

const version = assertVersion();
const generated = join(web, "generated");
const dist = join(web, "dist");
const args = process.argv.slice(2);
const outs = new Set([join(repo, "python/molgfx/static")]);
for (const argument of args) {
  if (!["--dev", "--no-wasm"].includes(argument))
    throw new Error("Unknown build argument: " + argument);
}
if (!args.includes("--no-wasm")) {
  execFileSync(
    "wasm-pack",
    [
      "build",
      join(repo, "crates/molgfx-wasm"),
      args.includes("--dev") ? "--dev" : "--release",
      "--target",
      "web",
      "--out-dir",
      generated,
      "--out-name",
      "molgfx_wasm",
      "--no-pack",
    ],
    { stdio: "inherit" },
  );
}
const runtime = ["molgfx_wasm.js", "molgfx_wasm_bg.wasm"];
for (const name of runtime)
  if (!existsSync(join(generated, name)))
    throw new Error("Missing generated runtime: " + name);
const tsc = join(web, "node_modules/typescript/bin/tsc");
execFileSync(process.execPath, [tsc, "-p", join(web, "tsconfig.json")], {
  stdio: "inherit",
});
rmSync(dist, { recursive: true, force: true });
mkdirSync(dist, { recursive: true });
await build({
  entryPoints: {
    index: join(web, "src/index.ts"),
    viewer: join(web, "src/viewer.ts"),
    anywidget: join(web, "src/entry/anywidget.ts"),
    react: join(web, "src/react/index.ts"),
  },
  outdir: dist,
  bundle: true,
  external: ["./molgfx_wasm.js", "react", "react/jsx-runtime"],
  plugins: [
    {
      name: "shared-wasm-assets",
      setup(builder) {
        builder.onResolve({ filter: /generated\/molgfx_wasm\.js$/ }, () => ({
          path: "./molgfx_wasm.js",
          external: true,
        }));
        builder.onLoad({ filter: /runtime\/runtime\.ts$/ }, (args) => ({
          contents: readFileSync(args.path, "utf8").replaceAll(
            "../../generated/molgfx_wasm_bg.wasm",
            "./molgfx_wasm_bg.wasm",
          ),
          loader: "ts",
        }));
      },
    },
  ],
  format: "esm",
  platform: "browser",
  target: ["es2022"],
  minify: true,
  sourcemap: true,
  legalComments: "none",
  charset: "utf8",
});
execFileSync(
  process.execPath,
  [
    tsc,
    "-p",
    join(web, "tsconfig.build.json"),
    "--noEmit",
    "false",
    "--declaration",
    "--emitDeclarationOnly",
    "--rootDir",
    join(web, "src"),
    "--outDir",
    join(dist, "types"),
  ],
  { stdio: "inherit" },
);
copyFileSync(join(web, "src/viewer.css"), join(dist, "viewer.css"));
for (const name of runtime)
  copyFileSync(join(generated, name), join(dist, name));
copyFileSync(join(repo, "LICENSE"), join(web, "LICENSE"));
const glue = readFileSync(join(dist, runtime[0]));
const wasm = readFileSync(join(dist, runtime[1]));
const contentKey = createHash("sha256")
  .update(glue)
  .update(Buffer.from([0]))
  .update(wasm)
  .digest("hex")
  .slice(0, 16);
function hashes(dir, prefix = "") {
  return Object.fromEntries(
    readdirSync(dir, { withFileTypes: true })
      .sort((a, b) => a.name.localeCompare(b.name))
      .flatMap((entry) => {
        const name = prefix + entry.name;
        return entry.isDirectory()
          ? Object.entries(hashes(join(dir, entry.name), name + "/"))
          : [
              [
                name,
                createHash("sha256")
                  .update(readFileSync(join(dir, entry.name)))
                  .digest("hex"),
              ],
            ];
      }),
  );
}
writeFileSync(
  join(dist, "runtime-manifest.json"),
  JSON.stringify(
    {
      available: true,
      version,
      glue: runtime[0],
      wasm: runtime[1],
      content_key: contentKey,
      sha256: hashes(dist),
    },
    null,
    2,
  ) + "\n",
);
checkArtifacts();
for (const out of outs) {
  rmSync(out, { recursive: true, force: true });
  cpSync(dist, out, { recursive: true });
}
console.log(
  "MolGFX " +
    version +
    ": " +
    (wasm.length / 1e6).toFixed(1) +
    " MB wasm; content key " +
    contentKey,
);

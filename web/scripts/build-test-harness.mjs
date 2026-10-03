import { build } from "esbuild";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { web } from "./version.mjs";
await build({
  entryPoints: [join(web, "src/tests/react-harness.ts")],
  outfile: join(web, "test-results/react-harness.js"),
  bundle: true,
  platform: "browser",
  format: "esm",
  target: "es2022",
  plugins: [
    {
      name: "test-runtime-assets",
      setup(builder) {
        builder.onResolve({ filter: /generated\/molgfx_wasm\.js$/ }, () => ({
          path: "/dist/molgfx_wasm.js",
          external: true,
        }));
        builder.onLoad({ filter: /runtime\/runtime\.ts$/ }, (args) => ({
          contents: readFileSync(args.path, "utf8").replaceAll(
            "../../generated/molgfx_wasm_bg.wasm",
            "/dist/molgfx_wasm_bg.wasm",
          ),
          loader: "ts",
        }));
      },
    },
  ],
});

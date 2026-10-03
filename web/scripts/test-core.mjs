import { build } from "esbuild";
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { web } from "./version.mjs";
const out = join(web, "test-results/core-tests.mjs");
mkdirSync(join(web, "test-results"), { recursive: true });
await build({
  entryPoints: [join(web, "src/tests/core-tests.ts")],
  bundle: true,
  platform: "node",
  format: "esm",
  target: "es2022",
  outfile: out,
});
execFileSync(process.execPath, [out], { stdio: "inherit" });

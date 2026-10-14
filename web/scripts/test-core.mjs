import { build } from "esbuild";
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { web } from "./version.mjs";
mkdirSync(join(web, "test-results"), { recursive: true });
for (const name of ["core-tests", "transport-tests", "adapter-tests", "frame-sequence-tests"]) {
  const out = join(web, `test-results/${name}.mjs`);
  await build({
    entryPoints: [join(web, `src/tests/${name}.ts`)],
    bundle: true,
    platform: "node",
    format: "esm",
    target: "es2022",
    outfile: out,
  });
  execFileSync(process.execPath, [out], { stdio: "inherit" });
}

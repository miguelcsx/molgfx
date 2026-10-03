import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { assertVersion, repo, web } from "../scripts/version.mjs";
import { checkArtifacts } from "../scripts/check-artifacts.mjs";

test("Cargo, Python and npm share a version", () => assert.equal(assertVersion(), JSON.parse(readFileSync(join(web, "package.json"))).version));
test("public package declarations and asset hashes are valid", () => assert.ok(checkArtifacts().available));
test("notebook receives the canonical npm assets unchanged", () => {
  const manifest = checkArtifacts();
  for (const name of ["runtime-manifest.json", ...Object.keys(manifest.sha256)]) {
    const bytes = readFileSync(join(web, "dist", name));
    assert.deepEqual(readFileSync(join(repo, "python/molgfx/static", name)), bytes);
  }
});

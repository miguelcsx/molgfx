import { build, preview } from "vite";
import { chromium } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { web } from "./version.mjs";
import { browserLaunchOptions } from "./browser-options.mjs";

const fixture = join(web, "test-results/bundler");
rmSync(fixture, { recursive: true, force: true });
mkdirSync(fixture, { recursive: true });
const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const packed = JSON.parse(
  execFileSync(
    npm,
    ["pack", web, "--pack-destination", fixture, "--json", "--silent"],
    { encoding: "utf8" },
  ),
);
const archive = join(fixture, packed[0].filename);
writeFileSync(
  join(fixture, "package.json"),
  JSON.stringify({ private: true, type: "module" }),
);
execFileSync(npm, ["install", "--ignore-scripts", "--omit=peer", archive], {
  cwd: fixture,
  stdio: "inherit",
});
execFileSync(
  process.execPath,
  [
    "--input-type=module",
    "-e",
    'await import("molgfx"); await import("molgfx/viewer"); await import("molgfx/anywidget"); await import("molgfx/react"); console.log("SSR-safe package imports")',
  ],
  { cwd: fixture, stdio: "inherit" },
);
writeFileSync(
  join(fixture, "index.html"),
  '<div id="viewer" style="width:640px;height:480px"></div><script type="module" src="/main.js"></script>',
);
writeFileSync(
  join(fixture, "main.js"),
  String.raw`
import { Viewer } from "molgfx";
import "molgfx/viewer.css";
window.ready = (async () => {
  const viewer = await Viewer.create(document.querySelector("#viewer"));
  const pdb = "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 20.00           C\nEND\n";
  await viewer.load(new TextEncoder().encode(pdb), { name: "fixture.pdb" });
  viewer.execute("show spacefill, all");
  viewer.focus("all");
  return viewer;
})();
`,
);
await build({
  root: fixture,
  configFile: false,
  build: { target: "esnext", assetsInlineLimit: 0 },
});
const assets = readdirSync(join(fixture, "dist/assets"));
if (!assets.some((name) => name.endsWith(".wasm")))
  throw new Error("Vite did not emit the wasm asset");
const server = await preview({
  root: fixture,
  configFile: false,
  preview: { host: "127.0.0.1", port: 4174, strictPort: true },
});
const browser = await chromium.launch(browserLaunchOptions);
try {
  const page = await browser.newPage();
  const errors = [];
  const wasm = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("response", (response) => {
    if (response.url().endsWith(".wasm")) wasm.push(response.status());
  });
  await page.goto("http://127.0.0.1:4174");
  await page.evaluate(() => window.ready);
  await page.waitForTimeout(750);
  await page.evaluate(async () => {
    const viewer = await window.ready;
    if (!document.querySelector("canvas")) throw new Error("Missing canvas");
    await viewer.dispose();
  });
  if (errors.length || !wasm.includes(200))
    throw new Error(
      "Bundled browser failed: " + JSON.stringify({ errors, wasm }),
    );
  console.log(
    "Vite production package: real viewer and emitted wasm loaded successfully",
  );
} finally {
  await browser.close();
  await server.close();
}

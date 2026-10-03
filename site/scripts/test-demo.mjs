import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdirSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, extname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
const { chromium } = createRequire(new URL("../../web/package.json", import.meta.url))("@playwright/test");
import { browserLaunchOptions } from "../../web/scripts/browser-options.mjs";

const site = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(site, "out");
const results = join(site, "test-results");
mkdirSync(results, { recursive: true });
for (const name of ["molgfx", "molgfx/viewer", "molgfx/anywidget", "molgfx/react"])
  await import(name);
for (const name of ["molgfx/dist/index.js", "molgfx/molgfx_wasm.js", "molgfx/dist/molgfx_wasm_bg.wasm"])
  await assert.rejects(import(name), { code: "ERR_PACKAGE_PATH_NOT_EXPORTED" });

function files(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}
const emitted = files(join(out, "_next")).filter((path) => path.endsWith(".wasm"));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const canonical = hash(readFileSync(join(site, "../web/dist/molgfx_wasm_bg.wasm")));
assert.ok(emitted.some((path) => hash(readFileSync(path)) === canonical), "Next must emit byte-identical canonical wasm");
const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json" };
const server = createServer((request, response) => {
  try {
    const url = new URL(request.url, "http://localhost");
    if (!url.pathname.startsWith("/molgfx/")) throw new Error("Wrong base path");
    let path = resolve(out, "." + decodeURIComponent(url.pathname.slice("/molgfx".length)));
    if (path !== out && !path.startsWith(out + sep)) throw new Error("Outside export");
    if (statSync(path).isDirectory()) path = join(path, "index.html");
    response.setHeader("Content-Type", mime[extname(path)] ?? "application/octet-stream");
    response.end(readFileSync(path));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
let browser;
try {
  browser = await chromium.launch(browserLaunchOptions);
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  const errors = [], wasm = [], privateRequests = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.includes("/runtime/")) privateRequests.push(request.url());
  });
  page.on("response", (response) => {
    if (response.url().endsWith(".wasm")) wasm.push({ url: response.url(), status: response.status() });
  });
  const structure = page.waitForResponse((response) => response.url() === "https://files.rcsb.org/download/4HHB.cif" && response.ok(), { timeout: 60000 });
  await Promise.all([page.goto(`http://127.0.0.1:${server.address().port}/molgfx/`), structure]);
  const canvas = page.locator('[aria-label="MolGFX interactive molecular canvas"] canvas');
  await canvas.waitFor({ timeout: 60000 });
  let colored = 0;
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    assert.deepEqual(await page.locator('[aria-label="Live browser SDK example"] [role="alert"]:visible').allTextContents(), [], "Live demo reported a renderer error");
    const image = await canvas.screenshot();
    colored = await page.evaluate(async (base64) => {
      const image = new Image();
      image.src = "data:image/png;base64," + base64;
      await image.decode();
      const sample = document.createElement("canvas");
      sample.width = image.width; sample.height = image.height;
      const context = sample.getContext("2d");
      context.drawImage(image, 0, 0);
      const pixels = context.getImageData(0, 0, sample.width, sample.height).data;
      let colored = 0;
      for (let i = 0; i < pixels.length; i += 4) {
        const max = Math.max(pixels[i], pixels[i + 1], pixels[i + 2]);
        const min = Math.min(pixels[i], pixels[i + 1], pixels[i + 2]);
        if (max - min > 40 && max > 80) colored++;
      }
      return colored;
    }, image.toString("base64"));
    if (colored > 500) break;
    await page.waitForTimeout(250);
  }
  assert.ok(colored > 500, "Real haemoglobin geometry must appear in the WebGPU canvas");
  assert.ok(wasm.some(({ url, status }) => url.includes("/molgfx/_next/") && status === 200), "Wasm must load through Next's base-prefixed emitted assets");
  assert.deepEqual(privateRequests, [], "Docs must not request a private runtime");
  await canvas.screenshot({ path: join(results, "demo.png") });
  await page.getByRole("link", { name: "Read the docs", exact: true }).click();
  await page.waitForURL("**/molgfx/docs/");
  assert.equal(await page.locator('[aria-label="MolGFX interactive molecular canvas"] canvas').count(), 0, "Navigation must unmount the viewer");
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ ssr: "public imports only", wasm, coloredPixels: colored, canonicalWasmSha256: canonical, teardown: "passed" }, null, 2));
} finally {
  if (browser) await browser.close();
  await new Promise((done) => server.close(done));
}

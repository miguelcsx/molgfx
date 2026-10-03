import { chromium } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { join, resolve, extname } from "node:path";
import { web } from "./version.mjs";
import { browserLaunchOptions } from "./browser-options.mjs";
const fixture = join(web, "test-results/next");
rmSync(fixture, { recursive: true, force: true });
mkdirSync(join(fixture, "app"), { recursive: true });
const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const [packed] = JSON.parse(
  execFileSync(
    npm,
    ["pack", web, "--pack-destination", fixture, "--json", "--silent"],
    { encoding: "utf8" },
  ),
);
writeFileSync(
  join(fixture, "package.json"),
  JSON.stringify({ private: true, type: "module" }),
);
execFileSync(
  npm,
  [
    "install",
    "--ignore-scripts",
    "--omit=peer",
    join(fixture, packed.filename),
  ],
  { cwd: fixture, stdio: "inherit" },
);
writeFileSync(
  join(fixture, "next.config.mjs"),
  'export default { output: "export" };',
);
writeFileSync(
  join(fixture, "app/layout.js"),
  "export default function Layout({children}) { return <html><body>{children}</body></html>; }",
);
writeFileSync(
  join(fixture, "app/page.js"),
  String.raw`
"use client";
import { useEffect, useRef } from "react";
import { Viewer } from "molgfx";
import "molgfx/viewer.css";
export default function Page() {
  const root = useRef(null);
  useEffect(() => {
    window.ready = (async () => {
      const viewer = await Viewer.create(root.current);
      await viewer.load(new TextEncoder().encode("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 20.00           C\nEND\n"), { name: "next.pdb" });
      viewer.execute("show spacefill, all"); viewer.focus("all");
      return viewer;
    })();
    return () => { void window.ready.then(viewer => viewer.dispose()); };
  }, []);
  return <div ref={root} style={{width:640,height:480}} />;
}
`,
);
execFileSync(
  process.execPath,
  [join(web, "node_modules/next/dist/bin/next"), "build", "--webpack"],
  {
    cwd: fixture,
    stdio: "inherit",
    env: { ...process.env, NEXT_TELEMETRY_DISABLED: "1" },
  },
);
const out = join(fixture, "out");
const server = createServer((request, response) => {
  const path = resolve(
    out,
    "." +
      new URL(request.url, "http://localhost").pathname.replace(
        /\/$/,
        "/index.html",
      ),
  );
  if (!path.startsWith(out + "/")) {
    response.writeHead(403).end();
    return;
  }
  try {
    response.setHeader(
      "Content-Type",
      {
        ".html": "text/html",
        ".js": "application/javascript",
        ".css": "text/css",
        ".wasm": "application/wasm",
      }[extname(path)] ?? "application/octet-stream",
    );
    response.end(readFileSync(path));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(4175, "127.0.0.1", resolve));
const browser = await chromium.launch(browserLaunchOptions);
try {
  const page = await browser.newPage();
  const errors = [],
    wasm = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("response", (response) => {
    if (response.url().endsWith(".wasm")) wasm.push(response.status());
  });
  await page.goto("http://127.0.0.1:4175");
  await page.waitForFunction(() => window.ready !== undefined);
  await page.evaluate(() => window.ready);
  await page.waitForTimeout(750);
  await page.evaluate(async () => {
    if (!document.querySelector("canvas")) throw new Error("No canvas");
    await (await window.ready).dispose();
  });
  if (errors.length || !wasm.includes(200))
    throw new Error("Next package failed: " + JSON.stringify({ errors, wasm }));
  console.log(
    "Next production package: SSR build and real emitted wasm viewer passed",
  );
} finally {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}

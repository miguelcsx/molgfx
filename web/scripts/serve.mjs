import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve, sep } from "node:path";
import { web } from "./version.mjs";
const mime = {
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
  ".json": "application/json",
  ".html": "text/html",
};
createServer(async (request, response) => {
  const path = resolve(
    web,
    "." + decodeURIComponent(new URL(request.url, "http://localhost").pathname),
  );
  if (!path.startsWith(web + sep)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const bytes = await readFile(path);
    response
      .writeHead(200, {
        "Content-Type": mime[extname(path)] ?? "application/octet-stream",
      })
      .end(bytes);
  } catch {
    response.writeHead(404).end();
  }
}).listen(4173, "127.0.0.1", () =>
  console.log("MolGFX test host http://127.0.0.1:4173"),
);

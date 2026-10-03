import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const web = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const repo = resolve(web, "..");
export function assertVersion() {
  const cargo = readFileSync(resolve(repo, "Cargo.toml"), "utf8");
  const python = readFileSync(resolve(repo, "pyproject.toml"), "utf8");
  const version = cargo.match(
    /\[workspace\.package\][\s\S]*?^version = "([^"]+)"/m,
  )?.[1];
  const pythonVersion = python.match(
    /\[project\][\s\S]*?^version = "([^"]+)"/m,
  )?.[1];
  const npm = JSON.parse(readFileSync(resolve(web, "package.json"), "utf8"));
  const lock = JSON.parse(
    readFileSync(resolve(web, "package-lock.json"), "utf8"),
  );
  if (
    !version ||
    pythonVersion !== version ||
    npm.name !== "molgfx" ||
    npm.version !== version ||
    lock.version !== version ||
    lock.packages[""].version !== version ||
    lock.name !== "molgfx"
  ) {
    throw new Error(
      "Cargo, Python and npm must declare the same MolGFX package version",
    );
  }
  return version;
}

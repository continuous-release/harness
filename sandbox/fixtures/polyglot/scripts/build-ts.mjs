#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function arg(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

const version = arg("version", "") || process.env.RELEASE_VERSION || "0.0.0";
const out = path.resolve(arg("out", "dist/polyglot.tgz"));
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "polyglot-ts-"));
const pkgDir = path.join(tmp, "ts");
fs.cpSync(path.join(root, "ts"), pkgDir, { recursive: true });

const pkgPath = path.join(pkgDir, "package.json");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
pkg.version = version;
fs.writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

const dist = path.join(tmp, "dist");
fs.mkdirSync(dist);
execFileSync("npm", ["pack", "--pack-destination", dist, "--silent"], {
  cwd: pkgDir,
  stdio: ["ignore", "ignore", "inherit"],
});

const tgz = fs.readdirSync(dist).find((file) => file.endsWith(".tgz"));
if (!tgz) throw new Error("npm pack produced no tarball");

fs.mkdirSync(path.dirname(out), { recursive: true });
fs.copyFileSync(path.join(dist, tgz), out);
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`packed ${out}`);

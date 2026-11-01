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
const out = path.resolve(arg("out", "dist/polyglot.whl"));
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "polyglot-python-"));
const pkgDir = path.join(tmp, "python");
fs.cpSync(path.join(root, "python"), pkgDir, { recursive: true });

const pyprojectPath = path.join(pkgDir, "pyproject.toml");
const pyproject = fs.readFileSync(pyprojectPath, "utf8");
fs.writeFileSync(
  pyprojectPath,
  pyproject.replace(/^version = ".*"$/m, `version = "${version}"`),
);

const offline = process.env.POLYGLOT_OFFLINE === "1";
const dist = path.join(tmp, "dist");
fs.mkdirSync(dist);
const args = ["-m", "pip", "wheel", ".", "--no-deps", "-w", dist];
if (offline) args.push("--no-build-isolation");
execFileSync(process.env.PYTHON ?? "python3", args, {
  cwd: pkgDir,
  stdio: ["ignore", "ignore", "inherit"],
  env: {
    ...process.env,
    PIP_DISABLE_PIP_VERSION_CHECK: "1",
    ...(offline ? { PIP_NO_INDEX: "1" } : {}),
  },
});

const wheel = fs.readdirSync(dist).find((file) => file.endsWith(".whl"));
if (!wheel) throw new Error("pip wheel produced no wheel");

fs.mkdirSync(path.dirname(out), { recursive: true });
fs.copyFileSync(path.join(dist, wheel), out);
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`built ${out}`);

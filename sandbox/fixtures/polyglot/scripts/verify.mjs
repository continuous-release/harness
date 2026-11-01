#!/usr/bin/env node
import crypto from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function arg(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function fail(message) {
  console.error(`verify: ${message}`);
  process.exit(1);
}

const dist = path.resolve(arg("dist", "dist"));
const manifestPath = path.join(dist, "manifest.json");
if (!fs.existsSync(manifestPath)) fail(`missing ${manifestPath}`);

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const contract = JSON.parse(
  fs.readFileSync(path.join(root, "contract/contract.json"), "utf8"),
);
const expected = `polyglot ${manifest.version} ${contract.message}`;

for (const artifact of manifest.artifacts) {
  const file = path.join(dist, artifact.name);
  if (!fs.existsSync(file)) fail(`missing artifact ${artifact.name}`);
  const bytes = fs.readFileSync(file);
  if (bytes.length !== artifact.size) {
    fail(`${artifact.name}: size ${bytes.length}, expected ${artifact.size}`);
  }
  const digest = crypto.createHash("sha256").update(bytes).digest("hex");
  if (digest !== artifact.sha256) {
    fail(`${artifact.name}: sha256 ${digest}, expected ${artifact.sha256}`);
  }
}

const rustBinary = path.join(dist, "polyglot");
if (!fs.existsSync(rustBinary)) fail("missing rust binary");
try {
  fs.accessSync(rustBinary, fs.constants.X_OK);
} catch {
  fs.chmodSync(rustBinary, 0o755);
}
if (!process.argv.includes("--skip-rust-run")) {
  const env = { ...process.env };
  delete env.RELEASE_VERSION;
  const rustOutput = execFileSync(rustBinary, { env }).toString().trim();
  if (rustOutput !== expected) {
    fail(`rust binary printed "${rustOutput}", expected "${expected}"`);
  }
}

const npmTarball = path.join(dist, "polyglot.tgz");
if (!fs.existsSync(npmTarball)) fail("missing npm tarball");
const npmPackage = JSON.parse(
  execFileSync("tar", ["-xOzf", npmTarball, "package/package.json"]).toString(),
);
if (npmPackage.version !== manifest.version) {
  fail(`npm version ${npmPackage.version}, expected ${manifest.version}`);
}

const wheel = path.join(dist, "polyglot.whl");
if (!fs.existsSync(wheel)) fail("missing python wheel");
const metadata = execFileSync("unzip", ["-p", wheel, "*/METADATA"]).toString();
const wheelVersion = metadata
  .split("\n")
  .find((line) => line.startsWith("Version: "))
  ?.slice("Version: ".length);
if (wheelVersion !== manifest.version) {
  fail(`wheel version ${wheelVersion}, expected ${manifest.version}`);
}

console.log(`verified ${manifest.version} (${manifest.artifacts.length} artifacts)`);

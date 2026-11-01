#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function arg(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function kindOf(name) {
  if (name.endsWith(".tgz")) return "npm";
  if (name.endsWith(".whl")) return "python";
  return "rust-binary";
}

const version = arg("version", "") || process.env.RELEASE_VERSION || "0.0.0";
const out = path.resolve(arg("out", "dist"));
const manifestPath = path.resolve(arg("manifest", path.join(out, "manifest.json")));
const contract = JSON.parse(
  fs.readFileSync(path.join(root, "contract/contract.json"), "utf8"),
);

const artifacts = [];
for (let index = 0; index < process.argv.length; index += 1) {
  if (process.argv[index] !== "--artifact") continue;
  const value = process.argv[index + 1];
  const separator = value.indexOf("=");
  if (separator < 0) throw new Error(`expected name=path, got ${value}`);
  const name = value.slice(0, separator);
  const file = path.resolve(value.slice(separator + 1));
  const bytes = fs.readFileSync(file);
  artifacts.push({
    name,
    kind: kindOf(name),
    size: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  });
}

artifacts.sort((left, right) => left.name.localeCompare(right.name));
fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
fs.writeFileSync(
  manifestPath,
  `${JSON.stringify(
    {
      name: "polyglot",
      version,
      message: contract.message,
      artifacts,
    },
    null,
    2,
  )}\n`,
);
console.log(`wrote ${manifestPath}`);

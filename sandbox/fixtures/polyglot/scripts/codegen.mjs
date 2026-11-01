#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const contract = JSON.parse(
  fs.readFileSync(path.join(root, "contract/contract.json"), "utf8"),
);
const message = JSON.stringify(contract.message);
const files = {
  "rust/src/contract.rs": `pub const MESSAGE: &str = ${message};\n`,
  "ts/src/contract.js": `export const MESSAGE = ${message};\n`,
  "python/src/polyglot/contract.py": `MESSAGE = ${message}\n`,
};

const check = process.argv.includes("--check");
const out = check ? fs.mkdtempSync(path.join(os.tmpdir(), "polyglot-codegen-")) : root;

for (const [rel, content] of Object.entries(files)) {
  const target = path.join(out, rel);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content);
}

if (!check) {
  console.log(`generated ${Object.keys(files).length} files`);
  process.exit(0);
}

let ok = true;
for (const [rel, content] of Object.entries(files)) {
  const committed = path.join(root, rel);
  if (!fs.existsSync(committed) || fs.readFileSync(committed, "utf8") !== content) {
    console.error(`stale generated file: ${rel}`);
    ok = false;
  }
}
fs.rmSync(out, { recursive: true, force: true });
if (!ok) process.exit(1);
console.log("codegen is up to date");

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const pkg = require("../package.json");

export const VERSION = process.env.RELEASE_VERSION ?? pkg.version;

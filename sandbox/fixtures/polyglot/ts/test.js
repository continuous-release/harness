import assert from "node:assert/strict";
import test from "node:test";

import { describe, MESSAGE, VERSION } from "./src/index.js";

test("describes the contract", () => {
  assert.equal(describe(), `polyglot ${VERSION} ${MESSAGE}`);
  assert.match(describe(), /hello from polyglot/);
});

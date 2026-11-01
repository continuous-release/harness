import { MESSAGE } from "./contract.js";
import { VERSION } from "./version.js";

export { MESSAGE, VERSION };

export function describe() {
  return `polyglot ${VERSION} ${MESSAGE}`;
}

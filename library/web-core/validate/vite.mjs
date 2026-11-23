// The build alias of the validation seam: `dcl-validate-impl` names the checking implementation, or the
// no-op one in a performance build (DCL_PERF=1). Every host of `validate/index.ts` sets it.
import { fileURLToPath } from "node:url";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

export const PERF = process.env.DCL_PERF === "1";

/** The checking implementation, as an alias map entry (Sites). */
export function validateAliasObject() {
  return { "dcl-validate-impl": here("./checked.ts") };
}

/** The implementation the build mode selects, as an alias array entry (ui3). */
export function validateImplAlias() {
  return { find: "dcl-validate-impl", replacement: here(PERF ? "./unchecked.ts" : "./checked.ts") };
}

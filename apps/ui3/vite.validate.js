import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { PERF, validateImplAlias } from "../../library/web-core/validate/vite.mjs";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

export { PERF };

// The *.stub.ts modules the perf aliases below point at are generated, not
// committed: render them from the real schema modules whenever a perf build
// (or dev server) loads this config.
if (PERF) execFileSync(process.execPath, [here("./scripts/gen-schema-stubs.mts")], { stdio: "inherit" });

const CATALYST_SCHEMAS = [
  "backpack",
  "communities",
  "events",
  "notifications",
  "places",
  "profile",
];

export function validateAlias() {
  const alias = [validateImplAlias()];
  if (PERF) {
    alias.push({
      find: /^(.*)\/generated\/bridge-schemas$/,
      replacement: here("./src/generated/bridge-schemas.stub.ts"),
    });
    alias.push({
      find: /^(?:\.{1,2}\/)+(?:.*\/)?persisted-schemas$/,
      replacement: here("./src/data/persisted-schemas.stub.ts"),
    });
    alias.push({
      find: "@web-core/auth/thirdwebSchema",
      replacement: here("./src/data/auth/thirdwebSchema.stub.ts"),
    });
    for (const name of CATALYST_SCHEMAS) {
      alias.push({
        find: new RegExp(`^(?:\\.{1,2}/)+(?:.*/)?schemas/${name}$`),
        replacement: here(`./src/data/catalyst/schemas/${name}.stub.ts`),
      });
    }
  }
  return alias;
}

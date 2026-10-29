import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

export const PERF = process.env.DCL_PERF === "1";

// The *.stub.ts modules the perf aliases below point at are generated, not
// committed: render them from the real schema modules whenever a perf build
// (or dev server) loads this config.
if (PERF) execFileSync(process.execPath, [here("./scripts/gen-schema-stubs.mts")], { stdio: "inherit" });

export function validateAliasObject() {
  return {
    "dcl-validate-impl": here("./src/validate/checked.ts"),
  };
}

const CATALYST_SCHEMAS = [
  "backpack",
  "communities",
  "events",
  "notifications",
  "places",
  "profile",
];

export function validateAlias() {
  const alias = [
    {
      find: "dcl-validate-impl",
      replacement: here(PERF ? "./src/validate/unchecked.ts" : "./src/validate/checked.ts"),
    },
  ];
  if (PERF) {
    alias.push({
      find: /^(.*)\/generated\/bridge-schemas$/,
      replacement: here("./src/generated/bridge-schemas.stub.ts"),
    });
    alias.push({
      find: /^(.*)\/generated\/editor-bus-schemas$/,
      replacement: here("./src/generated/editor-bus-schemas.stub.ts"),
    });
    alias.push({
      find: /^(?:\.{1,2}\/)+(?:.*\/)?persisted-schemas$/,
      replacement: here("./src/data/persisted-schemas.stub.ts"),
    });
    alias.push({
      find: /^(?:\.{1,2}\/)+(?:.*\/)?thirdwebSchema$/,
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

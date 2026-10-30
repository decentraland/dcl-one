import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const generated = fileURLToPath(new URL("../src/generated/", import.meta.url));
const missing = ["bridge", "webbridge", "editor-bus.ts"].filter((name) => !existsSync(generated + name));
if (missing.length > 0) {
  console.error(
    `Frontend contracts missing; restore apps/ui3/src/generated from this repository.`,
  );
  process.exit(1);
}

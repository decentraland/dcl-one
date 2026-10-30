// `pnpm typecheck` compiles ../ui3/src through the @ui alias. The generated
// Catalyst bindings are rendered, not committed, so verify them first.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const ui3 = fileURLToPath(new URL("../../ui3", import.meta.url));
const generated = ["bridge", "webbridge", "editor-bus.ts"].filter((name) => !existsSync(`${ui3}/src/generated/${name}`));
if (generated.length > 0) {
  console.error(
    `Frontend contracts missing; restore apps/ui3/src/generated from this repository.`,
  );
  process.exit(1);
}

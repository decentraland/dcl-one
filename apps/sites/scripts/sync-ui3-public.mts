// public/ <- ../ui3/public, after rendering the assets ui3 generates into its
// public/ (the nginx error pages). Runs as predev/prebuild and inside the Nix
// sites package, so nothing under ui3/public that a script produces is
// committed.
import { execFileSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ui3 = fileURLToPath(new URL("../../ui3", import.meta.url));
const src = join(ui3, "public");
const dst = fileURLToPath(new URL("../public", import.meta.url));

if (!existsSync(src)) {
  console.warn(`[sync:assets] ${src} not found \u{2014} skipping (ui3 public assets absent)`);
  process.exit(0);
}
execFileSync(process.execPath, [join(ui3, "scripts", "gen-error-pages.mts")], { cwd: ui3, stdio: "inherit" });
mkdirSync(dst, { recursive: true });
for (const entry of readdirSync(dst)) {
  rmSync(join(dst, entry), { recursive: true, force: true });
}
cpSync(src, dst, { recursive: true });
console.log("[sync:assets] copied ui3/public -> sites/public");

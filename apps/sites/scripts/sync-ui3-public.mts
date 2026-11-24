// public/ <- ../ui3/public, then the nginx error pages Sites renders itself
// (scripts/gen-error-pages.mts) into public/errors. Runs as predev/prebuild and
// inside the Nix sites package, so nothing a script produces under public/ is
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

mkdirSync(dst, { recursive: true });
if (existsSync(src)) {
  for (const entry of readdirSync(dst)) {
    rmSync(join(dst, entry), { recursive: true, force: true });
  }
  cpSync(src, dst, { recursive: true });
  console.log("[sync:assets] copied ui3/public -> sites/public");
} else {
  console.warn(`[sync:assets] ${src} not found \u{2014} skipping (ui3 public assets absent)`);
}
// After the copy, which replaces public/ whole.
execFileSync(process.execPath, [fileURLToPath(new URL("./gen-error-pages.mts", import.meta.url))], {
  cwd: fileURLToPath(new URL("..", import.meta.url)),
  stdio: "inherit",
});

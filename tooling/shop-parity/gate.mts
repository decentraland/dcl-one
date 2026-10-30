// The parity gate: every page×viewport listed in state/shop-parity/gate.json must still be identical
// to the reference (no differing pixel outside its masks and deliberate regions, no unexpected
// structural difference) on the recording the gate names. The gate is local, like the recording.
// Exit 1 names what changed.
//
//   node gate.mts [--build]      # check the gated pages
//   node gate.mts --promote      # capture every page and add the identical ones to the gate
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { capture, dclOneIdentity, defaultSet } from "./capture.mts";
import { GATE_FILE, STATE } from "./lib/paths.mts";
import { isIdentical } from "./lib/run.mts";

type Gate = { recording: string; pixelTolerance: number; pages: Record<string, string> };

const gate: Gate = existsSync(GATE_FILE) ? (JSON.parse(readFileSync(GATE_FILE, "utf8")) as Gate) : { recording: defaultSet(), pixelTolerance: 0, pages: {} };
const promote = process.argv.includes("--promote");
// A page is marked identical against a commit; uncommitted sources have none.
if (promote && dclOneIdentity().endsWith("+uncommitted")) {
  console.error("gate: commit or stash the changes in apps/ui3 and apps/sites before promoting");
  process.exit(1);
}
const gated = Object.keys(gate.pages);
const out = join(STATE, "gate", new Date().toISOString().replace(/[:.]/g, "-"));

if (!promote && gated.length === 0) {
  console.log(`gate: no page is marked identical yet (${GATE_FILE}); nothing to check`);
  process.exit(0);
}

const pages = promote ? undefined : [...new Set(gated.map((g) => g.split("@")[0]!))];
const viewports = promote ? undefined : [...new Set(gated.map((g) => g.split("@")[1]!))].join(",");
const results = await capture({ set: gate.recording, pages, viewports, out, build: process.argv.includes("--build"), strict: true, quiet: true });

if (promote) {
  const today = new Date().toISOString().slice(0, 10);
  for (const r of results) {
    const id = `${r.page}@${r.viewport}`;
    if (isIdentical(r, gate.pixelTolerance) && !gate.pages[id]) gate.pages[id] = today;
  }
  writeFileSync(GATE_FILE, `${JSON.stringify(gate, null, 2)}\n`);
  console.log(`gate: ${Object.keys(gate.pages).length} page×viewport marked identical; report ${join(out, "index.html")}`);
  process.exit(0);
}

let failed = 0;
for (const id of gated) {
  const r = results.find((x) => `${x.page}@${x.viewport}` === id);
  if (!r) {
    console.error(`gate: ${id} was not captured`);
    failed++;
    continue;
  }
  if (isIdentical(r, gate.pixelTolerance)) {
    console.log(`gate: ${id} identical`);
    continue;
  }
  failed++;
  const err = r.sides.reference.error || r.sides["dcl-one"].error;
  console.error(`gate: ${id} is no longer identical: ${r.pixels.diff} differing pixels${err ? `, error: ${err}` : ""}`);
  for (const l of r.lines.filter((_, i) => !r.structure[i]!.expected).slice(0, 40)) console.error(`  ${l}`);
}
console.log(`report: ${join(out, "index.html")}`);
process.exit(failed ? 1 : 0);

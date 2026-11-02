// The parity gate: every page×viewport listed in state/shop-parity/gate.json must still be identical
// to the reference (no differing pixel outside its masks and deliberate regions, no unexpected
// structural difference) on the recording the gate names. The gate is local, like the recording.
// Exit 1 names what changed.
//
//   node gate.mts [--build]      # check the gated pages
//   node gate.mts --promote      # capture every page and add the identical ones to the gate
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { capture, defaultSet } from "./capture.mts";
import { isCommitted } from "./lib/build.mts";
import { GATE_FILE, STATE } from "./lib/paths.mts";
import { gateFailures, isIdentical } from "./lib/verdict.mts";

type Gate = { recording: string; pixelTolerance: number; pages: Record<string, string> };

const gate: Gate = existsSync(GATE_FILE) ? (JSON.parse(readFileSync(GATE_FILE, "utf8")) as Gate) : { recording: defaultSet(), pixelTolerance: 0, pages: {} };
const promote = process.argv.includes("--promote");
// A page is marked identical against a commit: the build's sources and this tool's declarations both.
if (promote && !isCommitted()) {
  console.error("gate: commit or stash every change (and move apps/sites' uncommitted .env files aside) before promoting");
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
const results = await capture({ set: gate.recording, pages, viewports, out, build: process.argv.includes("--build"), quiet: true });

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

const failures = gateFailures(gated, results, gate.pixelTolerance);
for (const id of gated) {
  const f = failures.find((x) => x.id === id);
  if (!f) {
    console.log(`gate: ${id} identical`);
    continue;
  }
  console.error(`gate: ${id} is no longer identical:`);
  for (const reason of f.reasons) console.error(`  ${reason}`);
}
console.log(`report: ${join(out, "index.html")}`);
process.exit(failures.length ? 1 : 0);

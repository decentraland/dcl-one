// Record the API answers both sides read, once, from the production services (read-only GETs and
// public POST lookups), into state/shop-parity/recordings/<set>/. A recording holds real answers, so it
// stays on the machine that made it, out of git; each developer records their own.
//
//   node record.mts --set <name> [--pages overview,items] [--viewports desktop,mobile]
//   node record.mts --set <name> --fill     # replay; read what the replay could not answer, once
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { PAGES, type PageSpec } from "./pages.mts";
import { offerSynthesizer } from "./lib/offers.mts";
import { RECORDINGS, referencePin, ROOT, STATE } from "./lib/paths.mts";
import { fillMisses, Recording } from "./lib/recording.mts";
import { runAll, selectViewports } from "./lib/run.mts";
import { prepareReference } from "./reference.mts";

const arg = (name: string) => {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : undefined;
};
const set = arg("--set");
if (!set || !/^[a-z0-9-]+$/.test(set)) throw new Error("--set <name> (lowercase, digits, dashes) is required");
const dir = join(RECORDINGS, set);
const wanted = arg("--pages")?.split(",");
const specs = wanted ? PAGES.filter((p) => wanted.includes(p.id)) : PAGES;

if (process.argv.includes("--fill")) {
  // Replay the recording, then read each request it could not answer once from production.
  const replay = new Recording(dir);
  if (!replay.manifest) throw new Error(`no recording ${set} under ${RECORDINGS}`);
  await runAll(replay, specs, selectViewports(arg("--viewports")), { now: Date.parse(replay.manifest.recordedAt), targets: replay.manifest.targets, out: join(STATE, "fill-run", set) });
  const { failed } = await fillMisses(replay, new Recording(dir, { live: true }));
  if (failed.length) {
    console.error(`could not fill ${failed.length} request(s); what was filled is saved:\n${failed.map((f) => `  ${f}`).join("\n")}`);
    process.exitCode = 1;
  }
} else {
  prepareReference();
  const live = new Recording(dir, { live: true });
  live.synthesize = offerSynthesizer(live);
  const targets: Record<string, string> = { ...(live.manifest?.targets ?? {}) };
  const now = live.manifest?.recordedAt ? Date.parse(live.manifest.recordedAt) : Date.now();
  let current: PageSpec | null = null;
  const { results } = await runAll(live, specs, selectViewports(arg("--viewports")), {
    now,
    targets,
    out: join(STATE, "record-run", set),
    beforePage: async (spec) => {
      current = spec;
    },
    onPage: async (side, page) => {
      if (side === "reference" && current?.discover) Object.assign(targets, await current.discover(page));
    },
    onResult: (r) => console.log(`recorded ${r.page}@${r.viewport}: ${r.sides.reference.error ?? "reference ok"}, ${r.sides["dcl-one"].error ?? "dcl-one ok"}`),
  });
  const commit = spawnSync("git", ["rev-parse", "HEAD"], { cwd: ROOT, encoding: "utf8" }).stdout.trim();
  live.save({
    set,
    recordedAt: new Date(now).toISOString(),
    source: "production Decentraland services, read-only, through the pinned reference shop and the dcl-one UI",
    reference: referencePin().revision,
    dclOne: commit,
    targets,
  });
  const missing = results.flatMap((r) => r.misses);
  if (missing.length) console.log(`unanswered while recording (dcl-one-only routes): ${[...new Set(missing.map((m) => `${m.method} ${m.service} ${m.path}`))].join(", ")}`);
}

// An error answer replays like any other: both sides render the same error state and nothing is reported.
const errors = [...new Recording(dir).entries.values()].filter((e) => e.status >= 400 && e.status !== 404);
if (errors.length) console.warn(`warning: ${errors.length} answer(s) were recorded as errors; record again before trusting the pages that read them:\n${errors.map((e) => `  ${e.status} ${e.method} ${e.service} ${e.path}`).join("\n")}`);
console.log(`recording ${set}: ${dir}`);

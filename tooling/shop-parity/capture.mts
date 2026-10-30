// Compare apps/ui3/src/shop with the pinned reference shop over a local recording (record.mts).
//
//   node capture.mts [--set <recording>] [--pages overview,items] [--viewports desktop,mobile] [--out <dir>] [--build]
//
// Writes <out>/index.html (the report), run.json and summary.txt; default out is
// state/shop-parity/runs/<timestamp>/. Nothing is read from the network.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { PAGES } from "./pages.mts";
import { GATE_FILE, RECORDINGS, ROOT, SITES, STATE } from "./lib/paths.mts";
import { Recording } from "./lib/recording.mts";
import { summaryText, writeReport } from "./lib/report.mts";
import { runAll, selectViewports, type PageResult } from "./lib/run.mts";
import { prepareReference } from "./reference.mts";

export function defaultSet(): string {
  if (existsSync(GATE_FILE)) return (JSON.parse(readFileSync(GATE_FILE, "utf8")) as { recording: string }).recording;
  const sets = existsSync(RECORDINGS) ? readdirSync(RECORDINGS).sort() : [];
  if (!sets.length) throw new Error(`no recording under ${RECORDINGS}: run record.mts --set <name>`);
  return sets.at(-1)!;
}

/** The dcl-one build identity: the commit plus whether apps/ui3 or apps/sites differ from it. */
export function dclOneIdentity(): string {
  const head = spawnSync("git", ["rev-parse", "--short=12", "HEAD"], { cwd: ROOT, encoding: "utf8" }).stdout.trim();
  const dirty = spawnSync("git", ["status", "--porcelain", "--", "apps/ui3/src", "apps/sites/src"], { cwd: ROOT, encoding: "utf8" }).stdout.trim();
  return dirty ? `${head}+uncommitted` : head;
}

/** Which sources apps/sites/build was built from, written by buildSites(). */
const BUILD_STAMP = join(SITES, "build", ".shop-parity-source");

export function buildSites(): void {
  const identity = dclOneIdentity();
  const r = spawnSync("pnpm", ["build"], { cwd: SITES, stdio: "inherit" });
  if (r.status !== 0) throw new Error("sites build failed");
  writeFileSync(BUILD_STAMP, `${identity}\n`);
}

/**
 * `strict` (the gate) also refuses uncommitted sources it did not just build: a stamp of
 * `<commit>+uncommitted` cannot tell which uncommitted state it was.
 */
export async function capture(opts: { set: string; pages?: string[]; viewports?: string; out: string; build?: boolean; strict?: boolean; quiet?: boolean }): Promise<PageResult[]> {
  prepareReference();
  if (opts.build) buildSites();
  // Comparing a build of other sources would label its result with this commit.
  const identity = dclOneIdentity();
  const built = existsSync(BUILD_STAMP) ? readFileSync(BUILD_STAMP, "utf8").trim() : null;
  if (built !== identity) throw new Error(`apps/sites/build is ${built ? `of ${built}` : "not one this tool built"}, the sources are at ${identity}: pass --build`);
  if (opts.strict && !opts.build && identity.endsWith("+uncommitted")) throw new Error("apps/ui3 or apps/sites has uncommitted changes: pass --build so the build is of them");
  const recording = new Recording(join(RECORDINGS, opts.set));
  if (!recording.manifest) throw new Error(`no recording ${opts.set} under ${RECORDINGS}: run record.mts --set ${opts.set}`);
  const specs = opts.pages ? PAGES.filter((p) => opts.pages!.includes(p.id)) : PAGES;
  mkdirSync(opts.out, { recursive: true });
  const startedAt = new Date().toISOString();
  const { results, executable } = await runAll(recording, specs, selectViewports(opts.viewports), {
    now: Date.parse(recording.manifest.recordedAt),
    targets: recording.manifest.targets,
    out: opts.out,
    onResult: (r) => {
      if (!opts.quiet) console.log(`${r.identical ? "identical" : "different"} ${r.page}@${r.viewport}: ${r.pixels.diff} px, ${r.structure.filter((d) => !d.expected).length} structural`);
    },
  });
  writeReport(opts.out, results, {
    recording: opts.set,
    recordedAt: recording.manifest.recordedAt,
    reference: recording.manifest.reference,
    dclOne: identity,
    chromium: executable,
    startedAt,
  });
  return results;
}

if (import.meta.url === pathToFileURL(process.argv[1]!).href) {
  const arg = (name: string) => {
    const i = process.argv.indexOf(name);
    return i >= 0 ? process.argv[i + 1] : undefined;
  };
  const out = arg("--out") ?? join(STATE, "runs", new Date().toISOString().replace(/[:.]/g, "-"));
  const results = await capture({ set: arg("--set") ?? defaultSet(), pages: arg("--pages")?.split(","), viewports: arg("--viewports"), out, build: process.argv.includes("--build") });
  console.log(`\n${summaryText(results)}\nreport: ${join(out, "index.html")}`);
}

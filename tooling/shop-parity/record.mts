// Record the API answers both sides read, once, from the production services (read-only GETs and
// public POST lookups), then anonymize them into tooling/shop-parity/recordings/<set>/.
//
//   node record.mts --set <name> [--pages overview,items] [--viewports desktop,mobile]
//   node record.mts --set <name> --anonymize-only     # redo the anonymization from state/
//   node record.mts --set <name> --assets             # fetch the recording's external fonts
//   node record.mts --set <name> --fill               # replay; read what was missing once; re-anonymize
//
// The raw answers and the real→fake map stay in state/shop-parity/raw/<set>/ (gitignored).
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { PAGES, type PageSpec } from "./pages.mts";
import { anonymizeRecording, bundledAddresses, type ScrubMap } from "./lib/anonymize.mts";
import { offerSynthesizer } from "./lib/offers.mts";
import { RECORDINGS, referenceDist, referencePin, ROOT, STATE, UI3 } from "./lib/paths.mts";
import { Recording } from "./lib/recording.mts";
import { runAll, selectViewports } from "./lib/run.mts";
import { prepareReference } from "./reference.mts";

const arg = (name: string) => {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : undefined;
};
const set = arg("--set");
if (!set || !/^[a-z0-9-]+$/.test(set)) throw new Error("--set <name> (lowercase, digits, dashes) is required");
const rawDir = join(STATE, "raw", set);
const outDir = join(RECORDINGS, set);

if (process.argv.includes("--assets")) {
  const n = await new Recording(outDir).fetchAssets();
  console.log(`fetched ${n} asset(s)`);
  process.exit(0);
}

if (process.argv.includes("--fill")) {
  // Replay the committed recording, collect what it could not answer, map those requests back to
  // the real ones (the map is in state/), read them once from production into the raw recording.
  const committed = new Recording(outDir);
  if (!committed.manifest) throw new Error(`no committed recording ${set}`);
  await committed.fetchAssets();
  const wanted = arg("--pages")?.split(",");
  const specs = wanted ? PAGES.filter((p) => wanted.includes(p.id)) : PAGES;
  await runAll(committed, specs, selectViewports(arg("--viewports")), { now: Date.parse(committed.manifest.recordedAt), targets: committed.manifest.targets, out: join(STATE, "fill-run", set) });
  const map = JSON.parse(readFileSync(join(rawDir, "mapping.json"), "utf8")) as ScrubMap;
  const back = new Map(Object.entries(map.addresses).map(([real, fake]) => [fake, real]));
  const backCid = new Map(Object.entries(map.cids ?? {}).map(([real, fake]) => [fake, real]));
  const real = (s: string) => s.replace(/(?:baf[a-z2-7]{50,}|Qm[1-9A-HJ-NP-Za-km-z]{44})/g, (f) => backCid.get(f) ?? f).replace(/0xfa4e[0-9a-f]{34,36}/g, (f) => back.get(f) ?? f).replace(/0{24}(fa4e[0-9a-f]{36})/g, (w, f: string) => `${"0".repeat(24)}${(back.get(`0x${f}`) ?? `0x${f}`).slice(2)}`);
  const raw = new Recording(rawDir, { live: true });
  const seen = new Set<string>();
  for (const m of committed.misses) {
    if (m.service === "node" || m.service === "offers") continue;
    const request = m.request === undefined ? undefined : JSON.parse(real(JSON.stringify(m.request)));
    const key = `${m.method} ${m.service} ${real(m.path)}?${real(m.query)} ${JSON.stringify(request)}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const body = m.service === "rpc" ? Buffer.from(JSON.stringify({ jsonrpc: "2.0", id: 1, ...(request as object) })) : request === undefined ? undefined : Buffer.from(JSON.stringify(request));
    const answer = await raw.answer({ method: m.method, service: m.service, path: real(m.path), search: real(m.query), body, headers: { "content-type": "application/json", accept: "application/json" }, side: "fill" });
    console.log(`filled ${m.method} ${m.service} ${m.path} -> ${answer.status}`);
  }
  raw.save();
  process.argv.push("--anonymize-only");
}

if (!process.argv.includes("--anonymize-only")) {
  prepareReference();
  const raw = new Recording(rawDir, { live: true });
  raw.synthesize = offerSynthesizer(raw);
  const wanted = arg("--pages")?.split(",");
  const specs = wanted ? PAGES.filter((p) => wanted.includes(p.id)) : PAGES;
  const targets: Record<string, string> = { ...(raw.manifest?.targets ?? {}) };
  const now = raw.manifest?.recordedAt ? Date.parse(raw.manifest.recordedAt) : Date.now();
  let current: PageSpec | null = null;
  const { results } = await runAll(raw, specs, selectViewports(arg("--viewports")), {
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
  raw.save({
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

const raw = new Recording(rawDir);
const mapFile = join(rawDir, "mapping.json");
const previous = existsSync(mapFile) ? (JSON.parse(readFileSync(mapFile, "utf8")) as ScrubMap) : undefined;
rmSync(outDir, { recursive: true, force: true });
const keep = bundledAddresses([referenceDist(), join(UI3, "src", "shop")]);
const { map, leaks } = anonymizeRecording(raw, new Recording(outDir), keep, previous);
writeFileSync(mapFile, `${JSON.stringify(map, null, 1)}\n`);
if (leaks.length) {
  rmSync(outDir, { recursive: true, force: true });
  console.error(`anonymization left ${leaks.length} real value(s); nothing written:\n${leaks.slice(0, 20).join("\n")}`);
  process.exit(1);
}
console.log(`anonymized ${raw.entries.size} answers into ${outDir} (${Object.keys(map.addresses).length} addresses, ${Object.keys(map.names).length} names replaced; ${keep.size} bundled constants kept)`);

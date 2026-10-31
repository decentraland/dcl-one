// Which sources apps/sites/build was built from, so a run never compares a build of other sources
// under the current commit's name.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { childEnv } from "./env.mts";
import { ROOT, SITES } from "./paths.mts";

/** The harness itself is not a build input: editing it must not force a rebuild. */
const NOT_BUILT = ":(exclude)tooling/shop-parity";
/** Vite reads apps/sites/.env* at build time; the gitignored ones never show in git status. */
const IGNORED_ENV = ["ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--", "apps/sites/.env*"];

function git(root: string, args: string[]): Buffer {
  const r = spawnSync("git", args, { cwd: root, maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed in ${root}: ${r.stderr.toString().trim()}`);
  return r.stdout;
}

const files = (out: Buffer) => out.toString("utf8").split("\0").filter(Boolean);

/**
 * The commit, plus a hash of every uncommitted change that can reach the build: edits and untracked
 * files anywhere in the repository but this tool, and apps/sites' gitignored env files. Two different
 * uncommitted states never share an identity.
 */
export function dclOneIdentity(root = ROOT): string {
  const head = git(root, ["rev-parse", "--short=12", "HEAD"]).toString("utf8").trim();
  const diff = git(root, ["diff", "HEAD", "--binary", "--no-ext-diff", "--no-color", "--", ".", NOT_BUILT]);
  const extra = [...files(git(root, ["ls-files", "-z", "--others", "--exclude-standard", "--", ".", NOT_BUILT])), ...files(git(root, IGNORED_ENV))].sort();
  if (!diff.length && !extra.length) return head;
  const h = createHash("sha256").update(diff);
  for (const f of extra) {
    // git lists an untracked repository inside the tree as its directory: it counts by its own identity.
    const content = f.endsWith("/") ? dclOneIdentity(join(root, f)) : readFileSync(join(root, f));
    h.update("\0").update(f).update("\0").update(content);
  }
  return `${head}+${h.digest("hex").slice(0, 12)}`;
}

/** Whether the whole tree, this tool and apps/sites' gitignored env files included, is the commit. */
export function isCommitted(root = ROOT): boolean {
  return git(root, ["status", "--porcelain"]).length === 0 && git(root, IGNORED_ENV).length === 0;
}

type Stamp = { sources: string; node: string };

export function stampFile(sites = SITES): string {
  return join(sites, "build", ".shop-parity-source.json");
}

/** Build apps/sites with an allowlisted environment and stamp the build with what it was built from. */
export function buildSites(root = ROOT, sites = SITES): void {
  const sources = dclOneIdentity(root);
  // VITE_* and the like are baked into the bundle: a developer's own must not reach it.
  const r = spawnSync("pnpm", ["build"], { cwd: sites, stdio: "inherit", env: { ...childEnv(), CI: "1" } });
  if (r.status !== 0) throw new Error("sites build failed");
  writeFileSync(stampFile(sites), `${JSON.stringify({ sources, node: process.version } satisfies Stamp)}\n`);
}

/** Refuse a build that is not of the current sources (or was built by another Node). */
export function assertFreshBuild(sources: string, sites = SITES): void {
  const file = stampFile(sites);
  const stamp = existsSync(file) ? (JSON.parse(readFileSync(file, "utf8")) as Stamp) : null;
  if (!stamp) throw new Error(`apps/sites/build is not one this tool built, the sources are at ${sources}: pass --build`);
  if (stamp.sources !== sources) throw new Error(`apps/sites/build is of ${stamp.sources}, the sources are at ${sources}: pass --build`);
  if (stamp.node !== process.version) throw new Error(`apps/sites/build was built by Node ${stamp.node}, this is ${process.version}: pass --build`);
}

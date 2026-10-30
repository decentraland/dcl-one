import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SITES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SRC = path.join(SITES, "src");

// src/boundaries.json: layer -> the layers it may import through "@/<layer>/".
type Layer = { name: string; absRoot: string; mayImport: string[] };

const rules = JSON.parse(fs.readFileSync(path.join(SRC, "boundaries.json"), "utf8")) as Record<
  string,
  string[]
>;
const layers: Layer[] = Object.entries(rules).map(([name, mayImport]) => ({
  name,
  absRoot: path.join(SRC, name),
  mayImport,
}));
const byName = new Map(layers.map((l) => [l.name, l]));

const SOURCE_EXT = new Set([".ts", ".tsx", ".mts", ".mjs", ".js", ".jsx"]);

function walk(dir: string, out: string[] = []): string[] {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (SOURCE_EXT.has(path.extname(e.name))) out.push(p);
  }
  return out;
}

const SPEC_RE =
  /(?:^|[\s;}(])(?:import|export)\s+(?:type\s+)?(?:[\w*{},\s]*?\s+from\s+)?["']([^"']+)["']|import\s*\(\s*["']([^"']+)["']\s*\)/g;

function specifiers(src: string): string[] {
  const out: string[] = [];
  SPEC_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = SPEC_RE.exec(src))) {
    const s = m[1] ?? m[2];
    if (s) out.push(s);
  }
  return out;
}

function aliasTarget(spec: string): string | undefined {
  const m = /^@\/([^/]+)(?:\/|$)/.exec(spec);
  return m ? m[1] : undefined;
}

const owner = (abs: string) => layers.find((l) => abs.startsWith(l.absRoot + path.sep));
const rel = (abs: string) => path.relative(SITES, abs);

const WAIVED = new Set([
  "src/data/lib/auth/pair-store.test.ts -> @/routes/routes/internal.pair",
  "src/data/lib/catalyst/creator-hub/scene-drafts.test.ts -> @/routes/routes/api.creator-hub.drafts.$",
  "src/data/lib/catalyst/creator-hub/wearable-publish-collection.ts -> @/features/creator-hub/wearable-publish-collection/machine",
  "src/data/lib/catalyst/creator-hub/wearable-publish-collection.ts -> @/features/creator-hub/wearable-publish-collection/PublishCollectionWizard",
  "src/data/lib/catalyst/landings/rsvp.ts -> @/features/social/rsvp-event/machine",
  "src/data/lib/catalyst/landings/subscriptions.ts -> @/features/social/event-subscriptions/machine",
]);

function importViolations(): { key: string; message: string }[] {
  const out: { key: string; message: string }[] = [];
  for (const l of layers) {
    const allowed = new Set(l.mayImport);
    for (const file of walk(l.absRoot)) {
      for (const spec of specifiers(fs.readFileSync(file, "utf8"))) {
        const target = aliasTarget(spec);
        if (!target) continue;
        if (!byName.has(target)) {
          out.push({
            key: `${rel(file)} -> UNKNOWN ${spec}`,
            message: `${rel(file)}: "${spec}" names no layer in src/boundaries.json`,
          });
          continue;
        }
        if (target === l.name) {
          out.push({
            key: `${rel(file)} -> SELF ${spec}`,
            message: `${rel(file)}: uses @/${target} to import its own layer (use a relative path)`,
          });
          continue;
        }
        if (!allowed.has(target)) {
          out.push({
            key: `${rel(file)} -> ${spec}`,
            message: `${rel(file)}: ${l.name} may not import ${target} \u{2014} "${spec}"`,
          });
        }
      }
    }
  }
  return out;
}

describe("layer boundaries", () => {
  it("every layer directory exists and the rules form the intended layering", () => {
    expect(layers.map((l) => l.name).sort()).toEqual(["core", "data", "features", "routes"]);
    for (const l of layers) {
      expect(fs.existsSync(l.absRoot), `src/${l.name} exists`).toBe(true);
      for (const dep of l.mayImport) {
        expect(byName.has(dep), `${l.name} mayImport ${dep} names a real layer`).toBe(true);
      }
    }
  });

  it("no layer imports a layer it may not depend on", () => {
    const violations = importViolations();
    expect(violations.filter((v) => !WAIVED.has(v.key)).map((v) => v.message)).toEqual([]);
  });

  it("every waived upward import still exists (the waiver list only shrinks)", () => {
    const live = new Set(importViolations().map((v) => v.key));
    const stale = [...WAIVED].filter((k) => !live.has(k));
    expect(stale, "delete these from WAIVED \u{2014} the import they excused is gone").toEqual([]);
  });

  it("no relative import escapes its own layer", () => {
    const violations: string[] = [];
    for (const l of layers) {
      for (const file of walk(l.absRoot)) {
        for (const spec of specifiers(fs.readFileSync(file, "utf8"))) {
          if (!spec.startsWith(".")) continue;
          const abs = path.resolve(path.dirname(file), spec);
          if (abs.startsWith(l.absRoot + path.sep)) continue;
          if (!abs.startsWith(SRC + path.sep)) continue;
          const target = owner(abs);
          violations.push(
            `${rel(file)}: relative import "${spec}" escapes ${l.name} into ${target?.name ?? "another layer"} \u{2014} use its alias`,
          );
        }
      }
    }
    expect(violations).toEqual([]);
  });

  it("cross-layer filesystem coupling stays within the pinned inventory", () => {
    const roots = layers.map((l) => {
      const segs = path.relative(SITES, l.absRoot).split(path.sep);
      return {
        layer: l,
        joined: segs.map((s) => JSON.stringify(s)).join(","),
        slash: new RegExp(`(?:^|[^A-Za-z0-9_./-])${segs.join("/")}(?:/|["'\`\\s)]|$)`),
      };
    });
    const found: string[] = [];
    for (const l of layers) {
      const allowed = new Set(l.mayImport);
      for (const file of walk(l.absRoot)) {
        const src = fs.readFileSync(file, "utf8");
        const dense = src.replace(/\s+/g, "");
        for (const r of roots) {
          if (r.layer.name === l.name || allowed.has(r.layer.name)) continue;
          if (dense.includes(r.joined) || r.slash.test(src)) {
            found.push(`${rel(file)} -> ${r.layer.name}`);
          }
        }
      }
    }
    expect(found.sort()).toEqual([
      "src/core/lib/landings/creator-hub-download.server.ts -> data",
    ]);
  });

  it("only the routes layer holds SSR entry points and route modules", () => {
    const routes = byName.get("routes")!;
    for (const entry of ["root.tsx", "entry.client.tsx", "entry.server.tsx", "routes.ts"]) {
      expect(fs.existsSync(path.join(routes.absRoot, entry)), `routes layer owns ${entry}`).toBe(true);
    }
    for (const l of layers) {
      if (l.name === routes.name) continue;
      for (const file of walk(l.absRoot)) {
        expect(
          /\/\+types\//.test(fs.readFileSync(file, "utf8")),
          `${rel(file)} uses react-router route typegen outside the routes layer`,
        ).toBe(false);
      }
    }
  });
});

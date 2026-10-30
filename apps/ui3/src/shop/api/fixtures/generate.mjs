// Regenerates credits-wire.json from the pinned credits-server wire fixtures under
// tests/fixtures/shop (never edit the output by hand). `node generate.mjs --check` exits 1 when the
// committed file is stale. The mocks in ../fixtures.ts answer from this file only.
//
//   node ui3/src/shop/api/fixtures/generate.mjs [--check]

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "../../../../../..");
const wire = join(repo, "services/catalyst/tests/fixtures/shop/credits-server@465e628bc/wire");
export const OUTPUT = join(here, "credits-wire.json");

const SOURCES = [
  { file: "credits-authorize.json", sections: [["cases", "authorize"]] },
  { file: "credits-authorize-batch.json", sections: [["cases", "batch"]] },
  { file: "credits-authorize-cancel-submitted.json", sections: [["cancel", "cancel"], ["submitted", "submitted"]] },
];

function expand(value) {
  if (Array.isArray(value)) return value.map(expand);
  if (value && typeof value === "object") {
    if (typeof value.$repeat === "number" && "of" in value) return Array.from({ length: value.$repeat }, () => expand(value.of));
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, expand(v)]));
  }
  return value;
}

export function build(readText = (path) => readFileSync(path, "utf8")) {
  const out = { generatedBy: "ui3/src/shop/api/fixtures/generate.mjs", sources: [], setup: {}, cases: [] };
  for (const { file, sections } of SOURCES) {
    const path = join(wire, file);
    const text = readText(path);
    const json = JSON.parse(text);
    out.sources.push({ path: relative(repo, path), sha256: createHash("sha256").update(text).digest("hex") });
    for (const [section, route] of sections) {
      if (json.testSetup) out.setup[route] = json.testSetup;
      for (const c of json[section]) {
        const response = c.response ?? {};
        const body = response.body ?? response.bodyMatches ?? null;
        out.cases.push({
          route,
          name: c.name,
          source: c.source,
          auth: c.auth ?? null,
          state: c.state ?? null,
          request: "rawBody" in (c.request ?? {}) ? { rawBody: c.request.rawBody } : { body: expand(c.request?.body ?? null) },
          status: response.status,
          body: body && body.$notAsserted ? null : body,
          bodyAsserted: !(body && body.$notAsserted),
        });
      }
    }
  }
  return `${JSON.stringify(out, null, 2)}\n`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const next = build();
  if (process.argv.includes("--check")) {
    let current = "";
    try {
      current = readFileSync(OUTPUT, "utf8");
    } catch {
      // missing counts as stale
    }
    if (current !== next) {
      console.error(`${relative(process.cwd(), OUTPUT)} is stale: run node ${relative(process.cwd(), fileURLToPath(import.meta.url))}`);
      process.exit(1);
    }
  } else {
    writeFileSync(OUTPUT, next);
  }
}

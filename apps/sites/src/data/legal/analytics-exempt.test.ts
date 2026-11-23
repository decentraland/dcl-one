import { describe, expect, it } from "vitest";

import { isAnalyticsExemptPath } from "@/core/lib/telemetry/track";

import { LEGAL_ALIASES, LEGAL_SIDEBAR } from "./nav";

// The legacy site held back analytics for a session that started on any legal document except the
// Credits terms. Every path the site serves a document on must agree with that.
const TRACKED_DOCUMENTS = new Set(["credits"]);

const servedPaths = [
  ...LEGAL_SIDEBAR.flatMap(({ key, slug }) => [{ key, path: `/${slug}` }, { key, path: `/${key}` }]),
  ...Object.entries(LEGAL_ALIASES).map(([alias, key]) => ({ key, path: `/${alias}` })),
];

describe("the analytics exempt set and the served legal paths", () => {
  it.each(servedPaths)("$path follows its document ($key)", ({ key, path }) => {
    expect(isAnalyticsExemptPath(path)).toBe(!TRACKED_DOCUMENTS.has(key));
  });
});

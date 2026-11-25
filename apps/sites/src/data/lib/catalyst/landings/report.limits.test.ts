import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

import { REPORT_EVIDENCE_TYPES, REPORT_LIMITS } from "./report";

// The node refuses what is over its limits; the form refuses the same files before it asks for an upload.
const NODE = readFileSync(new URL("../../../../../../../services/catalyst/comms/src/gatekeeper/reports.rs", import.meta.url), "utf8");

const one = (pattern: RegExp): string => {
  const found = pattern.exec(NODE);
  if (!found?.[1]) throw new Error(`reports.rs has no ${pattern}`);
  return found[1];
};

it("holds the form's evidence limits to the node's", () => {
  expect(REPORT_LIMITS.maxFiles).toBe(Number(one(/^const MAX_FILES: usize = (\d+);/m)));
  expect(REPORT_LIMITS.maxFileSizeMb).toBe(Number(one(/^const MAX_FILE_BYTES: usize = (\d+) \* 1024 \* 1024;/m)));
});

it("holds the form's evidence types to the node's", () => {
  const list = one(/const ACCEPTED_TYPES: \[&str; \d+\] = \[([^\]]+)\];/);
  expect([...REPORT_EVIDENCE_TYPES]).toEqual([...list.matchAll(/"([^"]+)"/g)].map((m) => m[1]));
});

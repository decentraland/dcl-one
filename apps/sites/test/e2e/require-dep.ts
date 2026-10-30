import { appendFileSync } from "node:fs";

import { describe, it } from "vitest";

export const OPT_OUT = "ALLOW_SKIPPED_INTEGRATION";
export const SKIP_LOG = "CATALYRST_TESTGATE_SKIPLOG";

export function skipsAllowed(): boolean {
  const raw = process.env[OPT_OUT];
  if (raw === undefined || raw === "") return false;
  return raw !== "0" && raw.toLowerCase() !== "false";
}

export function refusal(requirement: string, detail: string): string {
  return [
    `integration dependency unavailable: ${requirement}`,
    `  ${detail}`,
    "  this test asserts nothing without it, so it fails instead of passing.",
    `  provide ${requirement}, or set ${OPT_OUT}=1 to let it skip on a machine that cannot run it.`,
  ].join("\n");
}

export function marker(
  name: string,
  requirement: string,
  detail: string,
): string {
  return `SKIPPED ${name}: ${requirement} unavailable (${detail}); ${OPT_OUT} is set`;
}

function recordSkip(name: string, requirement: string, detail: string): void {
  // eslint-disable-next-line no-console
  console.error(marker(name, requirement, detail));
  const path = process.env[SKIP_LOG];
  if (!path) return;
  try {
    appendFileSync(path, `${name}\t${requirement}\t${detail}\n`, "utf8");
  } catch {
  }
}

export function describeRequiring(
  present: boolean,
  requirement: string,
  detail: string,
): typeof describe {
  if (present) return describe;

  if (skipsAllowed()) {
    const failing = (name: string, body?: unknown): void => {
      recordSkip(name, requirement, detail);
      (describe.skip as unknown as (n: string, b?: unknown) => void)(name, body);
    };
    return failing as unknown as typeof describe;
  }

  const failing = (name: string, _body?: unknown): void => {
    describe(name, () => {
      it(`cannot run: ${requirement} is unavailable`, () => {
        throw new Error(refusal(requirement, detail));
      });
    });
  };
  return failing as unknown as typeof describe;
}

export const PG_REQUIREMENT =
  "a v2 node and its lab database for the sites e2e suite (SITES_E2E_NODE_URL + SITES_E2E_PG_URL)";

export function describeRequiringPg(): typeof describe {
  return describeRequiring(
    process.env.SITES_E2E_PG === "ready",
    PG_REQUIREMENT,
    "globalSetup did not mark the fixture ready; see tests/README.md section 2 " +
      "and point SITES_E2E_NODE_URL / SITES_E2E_PG_URL at the running node and its database",
  );
}

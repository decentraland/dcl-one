import { Pool } from "pg";

import { seedPlaces } from "./seed";

// The fixture is a running v2 node on the lab Postgres (tests/README.md
// section 2): `catalyst schema apply` into this agent's own database, then
// `catalyst serve --serve-unsynced`. Both must be named; nothing is provisioned
// here and nothing skips silently.
const NODE_VAR = "SITES_E2E_NODE_URL";
const PG_VAR = "SITES_E2E_PG_URL";

function required(name: string, hint: string): string {
  const v = process.env[name]?.trim();
  if (!v) {
    throw new Error(
      `integration dependency missing: ${name} is not set\n  ${hint}\n` +
        "  the sites e2e suite runs against a real v2 node and its database; it does not skip.",
    );
  }
  return v.replace(/\/$/, "");
}

function unusable(what: string, detail: string): Error {
  return new Error(
    `integration dependency configured but unusable: ${what}\n  ${detail}\n` +
      "  ALLOW_SKIPPED_INTEGRATION does not cover this: a dependency you explicitly " +
      "pointed the suite at must work.",
  );
}

async function checkNode(nodeUrl: string): Promise<void> {
  let res: Response;
  try {
    res = await fetch(`${nodeUrl}/about`, { headers: { accept: "application/json" } });
  } catch (e) {
    throw unusable(`v2 node at ${nodeUrl}`, (e as Error).message);
  }
  if (res.status !== 200) {
    throw unusable(
      `v2 node at ${nodeUrl}`,
      `GET /about answered ${res.status}; start it with \`catalyst serve --serve-unsynced\` ` +
        "(a plain `serve` stays Bootstrapping and answers 503)",
    );
  }
}

async function checkSchemaAndSeed(pgUrl: string): Promise<void> {
  const pool = new Pool({ connectionString: pgUrl, max: 2 });
  try {
    const probe = await pool.query<{ place: string | null; events: string | null }>(
      "SELECT to_regclass('explore.place')::text AS place, to_regclass('telemetry.events')::text AS events",
    );
    const row = probe.rows[0];
    if (!row?.place || !row?.events) {
      throw new Error(
        "explore.place or telemetry.events is missing: apply the v2 schema first (`catalyst schema apply` with DATABASE_URL set to this database)",
      );
    }
    const n = await seedPlaces(pool);
    console.log(`[e2e] seeded ${n} places into explore.place`);
    await pool.query("TRUNCATE telemetry.events RESTART IDENTITY");
    console.log("[e2e] cleared telemetry.events");
  } catch (e) {
    throw unusable(`lab postgres at ${PG_VAR}`, (e as Error).message);
  } finally {
    await pool.end();
  }
}

export async function setup(): Promise<void> {
  const nodeUrl = required(
    NODE_VAR,
    "e.g. SITES_E2E_NODE_URL=http://127.0.0.1:5210 (the `catalyst serve --serve-unsynced` listener)",
  );
  const pgUrl = required(
    PG_VAR,
    "e.g. SITES_E2E_PG_URL=postgresql://lab@/<your database>?host=/tmp/labpg&port=54318 (the database `catalyst schema apply` filled)",
  );
  await checkNode(nodeUrl);
  await checkSchemaAndSeed(pgUrl);

  process.env.CATALYST_URL = nodeUrl;
  process.env.CATALYST_DATABASE_URL = pgUrl;
  process.env.TELEMETRY_DATABASE_URL = pgUrl;
  process.env.SITES_E2E_PG = "ready";
}

export async function teardown(): Promise<void> {}

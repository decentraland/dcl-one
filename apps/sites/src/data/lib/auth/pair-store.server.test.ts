import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { Pool } from "pg";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";

import { createPgPairStore } from "./pair-store.server";
import type { PairStore } from "./pair-store.server";

// Needs a PostgreSQL the test may create a database in (the variable the catalyst tests read).
// The suite creates its own scratch database from schemas/11_explore_api.sql and drops it.
const ADMIN_URL = process.env.CATALYST_TEST_DATABASE_URL;
const SCHEMA = fileURLToPath(new URL("../../../../../../schemas/11_explore_api.sql", import.meta.url));

const input = { ephemeral: "0x" + "1".repeat(40), expiration: "2030-01-01T00:00:00.000Z", message: "m" };

async function mustCreate(store: PairStore) {
  const s = await store.create(input);
  if (!s) throw new Error("expected create() to return a session");
  return s;
}

describe.skipIf(!ADMIN_URL)("pair store on PostgreSQL", () => {
  const dbName = `sites_pair_test_${randomBytes(6).toString("hex")}`;
  let dbUrl = "";
  const pools: Pool[] = [];

  function newPool(): Pool {
    const p = new Pool({ connectionString: dbUrl, max: 5 });
    pools.push(p);
    return p;
  }

  beforeAll(async () => {
    const admin = new Pool({ connectionString: ADMIN_URL, max: 1 });
    try {
      await admin.query(`CREATE DATABASE ${dbName}`);
    } finally {
      await admin.end();
    }
    const u = new URL(ADMIN_URL!);
    u.pathname = `/${dbName}`;
    dbUrl = u.toString();
    const setup = new Pool({ connectionString: dbUrl, max: 1 });
    try {
      await setup.query(readFileSync(SCHEMA, "utf8"));
    } finally {
      await setup.end();
    }
  });

  afterEach(async () => {
    const p = new Pool({ connectionString: dbUrl, max: 1 });
    await p.query("DELETE FROM explore_api.pair_sessions");
    await p.end();
  });

  afterAll(async () => {
    await Promise.all(pools.map((p) => p.end()));
    const admin = new Pool({ connectionString: ADMIN_URL, max: 1 });
    try {
      await admin.query(`DROP DATABASE IF EXISTS ${dbName} WITH (FORCE)`);
    } finally {
      await admin.end();
    }
  });

  it("shares sessions across replicas: create on one, complete on another, poll on the first", async () => {
    const a = createPgPairStore(newPool());
    const b = createPgPairStore(newPool());
    const s = await mustCreate(a);
    expect(s.expiresAt - s.createdAt).toBe(5 * 60_000);

    expect(await a.poll(s.id, s.pollToken)).toEqual({ state: "pending" });
    const seen = await b.get(s.id);
    expect(seen).toMatchObject({ id: s.id, ephemeral: input.ephemeral, message: "m", completed: null });
    expect(seen).not.toHaveProperty("pollToken");

    expect(await b.complete(s.id, "0xabc", "0xsig")).toBe("ok");
    expect(await a.complete(s.id, "0xdef", "0xother")).toBe("already");
    expect((await a.get(s.id))?.completed).toEqual({ signer: "0xabc", signature: "0xsig" });

    expect(await b.poll(s.id, "A".repeat(43))).toEqual({ state: "forbidden" });
    expect(await a.poll(s.id, s.pollToken)).toEqual({
      state: "completed",
      signer: "0xabc",
      signature: "0xsig",
    });
    expect(await b.poll(s.id, s.pollToken)).toEqual({ state: "missing" });
    expect(await a.get(s.id)).toBeNull();
  });

  it("stores the poll token only as its sha256", async () => {
    const pool = newPool();
    const s = await mustCreate(createPgPairStore(pool));
    const { rows } = await pool.query(
      "SELECT poll_token_sha256 = sha256(convert_to($2, 'UTF8')) AS hashed FROM explore_api.pair_sessions WHERE id = $1",
      [s.id, s.pollToken],
    );
    expect(rows).toEqual([{ hashed: true }]);
  });

  it("completes a session once when replicas race, and releases it to one poll", async () => {
    const stores = Array.from({ length: 6 }, () => createPgPairStore(newPool()));
    const s = await mustCreate(stores[0]);
    const results = await Promise.all(
      stores.flatMap((st, i) => [st.complete(s.id, `0x${i}`, `0xsig${i}`), st.complete(s.id, `0x${i}b`, "0xs")]),
    );
    expect(results.filter((r) => r === "ok")).toHaveLength(1);
    expect(results.filter((r) => r === "already")).toHaveLength(results.length - 1);

    const polls = await Promise.all(stores.map((st) => st.poll(s.id, s.pollToken)));
    expect(polls.filter((p) => p.state === "completed")).toHaveLength(1);
    expect(polls.filter((p) => p.state === "missing")).toHaveLength(polls.length - 1);
  });

  it("keeps the cap of 500 live sessions under concurrent creates and frees room as rows expire", async () => {
    const a = createPgPairStore(newPool());
    const b = createPgPairStore(newPool());
    const created = await Promise.all(
      Array.from({ length: 520 }, (_, i) => (i % 2 ? a : b).create(input)),
    );
    expect(created.filter(Boolean)).toHaveLength(500);
    expect(await a.create(input)).toBeNull();
    const first = created.find(Boolean)!;
    expect(await b.get(first.id)).not.toBeNull();

    // A cancelled session frees its slot; a wrong token cancels nothing.
    expect(await a.cancel(first.id, "A".repeat(43))).toBe(false);
    expect(await a.cancel(first.id, first.pollToken)).toBe(true);
    expect(await b.get(first.id)).toBeNull();
    expect(await b.create(input)).not.toBeNull();
    expect(await b.create(input)).toBeNull();
  });

  it("expires sessions: get hides them, complete and poll answer expired, and the next create sweeps them", async () => {
    const pool = newPool();
    const live = createPgPairStore(pool);
    const stale = createPgPairStore(pool, -1);

    const s = await mustCreate(stale);
    expect(await live.get(s.id)).toBeNull();
    expect(await live.complete(s.id, "0xabc", "0xsig")).toBe("expired");
    expect(await live.complete(s.id, "0xabc", "0xsig")).toBe("missing");

    const t = await mustCreate(stale);
    expect(await live.poll(t.id, "A".repeat(43))).toEqual({ state: "forbidden" });
    expect(await live.poll(t.id, t.pollToken)).toEqual({ state: "expired" });
    expect(await live.poll(t.id, t.pollToken)).toEqual({ state: "missing" });

    for (let i = 0; i < 3; i++) await mustCreate(stale);
    await mustCreate(live);
    const { rows } = await pool.query<{ n: string }>("SELECT count(*) AS n FROM explore_api.pair_sessions");
    expect(Number(rows[0].n)).toBe(1);
  });
});

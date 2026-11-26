import { Pool } from "pg";

/** The catalyst PostgreSQL this Sites process reads places from. */
export function catalystDatabaseUrl(): string | undefined {
  if (typeof process === "undefined") return undefined;
  return (
    process.env.CATALYST_DATABASE_URL ||
    process.env.PLACES_PG_COMPONENT_PSQL_CONNECTION_STRING ||
    undefined
  );
}

export function catalystDatabaseConfigured(): boolean {
  return Boolean(catalystDatabaseUrl());
}

/**
 * The database that holds the pairing sessions: `CATALYST_DATABASE_URL` only. The legacy
 * `PLACES_PG_COMPONENT_PSQL_CONNECTION_STRING` may name a read-only or places-only database,
 * where every pairing write would fail.
 */
export function pairingDatabaseUrl(): string | undefined {
  if (typeof process === "undefined") return undefined;
  return process.env.CATALYST_DATABASE_URL || undefined;
}

/** A pool whose idle connections can drop (a failover, a restart) without ending the process,
 *  and whose checkout gives up when the database cannot be reached. */
function newPool(name: string, connectionString: string | undefined, max: number, statementTimeoutMs: number): Pool {
  const created = new Pool({
    connectionString,
    max,
    statement_timeout: statementTimeoutMs,
    idle_in_transaction_session_timeout: 30_000,
    idleTimeoutMillis: 30_000,
    connectionTimeoutMillis: 5_000,
  });
  // An idle client's error is emitted on the pool; unhandled, it would end the process.
  created.on("error", (err) => {
    console.error(`[${name}] idle database connection failed: ${err.message}`);
  });
  return created;
}

let placesPool: Pool | null = null;
let pairPool: Pool | null = null;

/** The places reads' pool: few connections, statements up to 60 s. */
export function catalystPool(): Pool {
  placesPool ??= newPool("catalyst-db", catalystDatabaseUrl(), 5, 60_000);
  return placesPool;
}

/** The pairing sessions' pool, apart from the places reads so a slow places query never holds
 *  the connection a poll waits for. Each pairing statement is a short keyed read or write. */
export function pairingPool(): Pool {
  pairPool ??= newPool("pair-db", pairingDatabaseUrl(), 4, 5_000);
  return pairPool;
}

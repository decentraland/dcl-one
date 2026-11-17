import { Pool } from "pg";

/** The catalyst PostgreSQL this Sites process reads (places) and writes (pairing sessions). */
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

let pool: Pool | null = null;

/** One pool per process, shared by every Sites feature that talks to the catalyst database. */
export function catalystPool(): Pool {
  if (!pool) {
    pool = new Pool({
      connectionString: catalystDatabaseUrl(),
      max: 5,
      statement_timeout: 60_000,
      idle_in_transaction_session_timeout: 30_000,
      idleTimeoutMillis: 30_000,
    });
  }
  return pool;
}

import { createHash, randomBytes, timingSafeEqual } from "node:crypto";

import type { Pool } from "pg";

import { pairingDatabaseUrl, pairingPool } from "../catalyst/pool.server";

const PAIR_TTL_MS = 5 * 60_000;
const MAX_SESSIONS = 500;

type PairCompletion = { signer: string; signature: string };

/** What `get` returns: the session without its poll token, which only the creator holds. */
export type PairSessionView = {
  id: string;
  ephemeral: string;
  expiration: string;
  message: string;
  createdAt: number;
  expiresAt: number;
  completed: PairCompletion | null;
};

/** What `create` returns: the view plus the poll token, handed to the desktop once. */
export type PairSession = PairSessionView & { pollToken: string };

export type PairPollResult =
  | { state: "missing" }
  | { state: "forbidden" }
  | { state: "expired" }
  | { state: "pending" }
  | ({ state: "completed" } & PairCompletion);

export type PairCompleteResult = "ok" | "missing" | "expired" | "already";

export type PairStore = {
  create(input: {
    ephemeral: string;
    expiration: string;
    message: string;
  }): Promise<PairSession | null>;
  get(id: string): Promise<PairSessionView | null>;
  complete(id: string, signer: string, signature: string): Promise<PairCompleteResult>;
  poll(id: string, pollToken: string): Promise<PairPollResult>;
  cancel(id: string, pollToken: string): Promise<boolean>;
};

function newId(): string {
  return randomBytes(16).toString("base64url");
}

function newPollToken(): string {
  return randomBytes(32).toString("base64url");
}

function tokenHash(token: string): Buffer {
  return createHash("sha256").update(token).digest();
}

/** Compares fixed-length sha256 digests, so neither the length nor the content of the token leaks. */
function tokenMatches(token: string, storedHash: Buffer): boolean {
  const h = tokenHash(token);
  return storedHash.length === h.length && timingSafeEqual(h, storedHash);
}

/** One process only: for tests and for a Sites run without a catalyst database. */
export function createMemoryPairStore(ttlMs = PAIR_TTL_MS): PairStore {
  type Stored = PairSessionView & { pollTokenHash: Buffer };
  const sessions = new Map<string, Stored>();

  function sweep(): void {
    const now = Date.now();
    for (const [id, s] of sessions) {
      if (now > s.expiresAt) sessions.delete(id);
    }
  }

  function view(s: Stored): PairSessionView {
    const { pollTokenHash: _hash, ...rest } = s;
    return { ...rest, completed: s.completed ? { ...s.completed } : null };
  }

  return {
    async create(input) {
      sweep();
      if (sessions.size >= MAX_SESSIONS) return null;
      const now = Date.now();
      const pollToken = newPollToken();
      const stored: Stored = {
        id: newId(),
        pollTokenHash: tokenHash(pollToken),
        ephemeral: input.ephemeral,
        expiration: input.expiration,
        message: input.message,
        createdAt: now,
        expiresAt: now + ttlMs,
        completed: null,
      };
      sessions.set(stored.id, stored);
      return { ...view(stored), pollToken };
    },
    async get(id) {
      sweep();
      const s = sessions.get(id);
      return s ? view(s) : null;
    },
    async complete(id, signer, signature) {
      sweep();
      const s = sessions.get(id);
      if (!s) return "missing";
      if (Date.now() > s.expiresAt) {
        sessions.delete(id);
        return "expired";
      }
      if (s.completed) return "already";
      s.completed = { signer, signature };
      return "ok";
    },
    async poll(id, pollToken) {
      sweep();
      const s = sessions.get(id);
      if (!s) return { state: "missing" };
      if (!tokenMatches(pollToken, s.pollTokenHash)) return { state: "forbidden" };
      if (Date.now() > s.expiresAt) {
        sessions.delete(id);
        return { state: "expired" };
      }
      if (!s.completed) return { state: "pending" };
      sessions.delete(id);
      return { state: "completed", ...s.completed };
    },
    async cancel(id, pollToken) {
      const s = sessions.get(id);
      if (!s || !tokenMatches(pollToken, s.pollTokenHash)) return false;
      sessions.delete(id);
      return true;
    },
  };
}

/** Serializes creates across every Sites replica, so the session cap holds under concurrency. */
const CREATE_LOCK_SQL = "SELECT pg_advisory_xact_lock(hashtextextended('sites-pair-sessions', 0))";

const VIEW_COLUMNS = `id, ephemeral, expiration, message,
  (extract(epoch FROM created_at) * 1000)::float8 AS created_ms,
  (extract(epoch FROM expires_at) * 1000)::float8 AS expires_ms,
  completed_signer, completed_signature`;

type ViewRow = {
  id: string;
  ephemeral: string;
  expiration: string;
  message: string;
  created_ms: number;
  expires_ms: number;
  completed_signer: string | null;
  completed_signature: string | null;
};

function viewFromRow(r: ViewRow): PairSessionView {
  return {
    id: r.id,
    ephemeral: r.ephemeral,
    expiration: r.expiration,
    message: r.message,
    createdAt: Math.round(Number(r.created_ms)),
    expiresAt: Math.round(Number(r.expires_ms)),
    completed:
      r.completed_signer !== null && r.completed_signature !== null
        ? { signer: r.completed_signer, signature: r.completed_signature }
        : null,
  };
}

/**
 * Sessions in `explore_api.pair_sessions`, shared by every Sites replica on the database. Every
 * transition is one conditional statement, so two replicas never both complete or both release a
 * session; creates take a transaction-scoped advisory lock to keep the cap of 500 live sessions and
 * delete the expired rows first. Expiry is the database clock (`now()`).
 */
export function createPgPairStore(pool: Pool, ttlMs = PAIR_TTL_MS): PairStore {
  return {
    async create(input) {
      const pollToken = newPollToken();
      const client = await pool.connect();
      try {
        await client.query("BEGIN");
        await client.query(CREATE_LOCK_SQL);
        await client.query("DELETE FROM explore_api.pair_sessions WHERE expires_at <= now()");
        const res = await client.query<ViewRow>(
          `INSERT INTO explore_api.pair_sessions
             (id, poll_token_sha256, ephemeral, expiration, message, created_at, expires_at)
           SELECT $1, $2, $3, $4, $5, now(), now() + make_interval(secs => $6::float8 / 1000)
           WHERE (SELECT count(*) FROM explore_api.pair_sessions) < $7
           RETURNING ${VIEW_COLUMNS}`,
          [
            newId(),
            tokenHash(pollToken),
            input.ephemeral,
            input.expiration,
            input.message,
            ttlMs,
            MAX_SESSIONS,
          ],
        );
        await client.query("COMMIT");
        const row = res.rows[0];
        return row ? { ...viewFromRow(row), pollToken } : null;
      } catch (err) {
        await client.query("ROLLBACK").catch(() => {});
        throw err;
      } finally {
        client.release();
      }
    },

    async get(id) {
      const res = await pool.query<ViewRow>(
        `SELECT ${VIEW_COLUMNS} FROM explore_api.pair_sessions WHERE id = $1 AND expires_at > now()`,
        [id],
      );
      return res.rows[0] ? viewFromRow(res.rows[0]) : null;
    },

    async complete(id, signer, signature) {
      const done = await pool.query(
        `UPDATE explore_api.pair_sessions
            SET completed_signer = $2, completed_signature = $3
          WHERE id = $1 AND completed_signer IS NULL AND expires_at > now()
          RETURNING id`,
        [id, signer, signature],
      );
      if (done.rowCount) return "ok";
      const res = await pool.query<{ expired: boolean }>(
        "SELECT expires_at <= now() AS expired FROM explore_api.pair_sessions WHERE id = $1",
        [id],
      );
      const row = res.rows[0];
      if (!row) return "missing";
      if (row.expired) {
        await pool.query(
          "DELETE FROM explore_api.pair_sessions WHERE id = $1 AND expires_at <= now()",
          [id],
        );
        return "expired";
      }
      return "already";
    },

    async poll(id, pollToken) {
      const res = await pool.query<{ poll_token_sha256: Buffer; expired: boolean; completed: boolean }>(
        `SELECT poll_token_sha256, expires_at <= now() AS expired,
                completed_signer IS NOT NULL AS completed
           FROM explore_api.pair_sessions WHERE id = $1`,
        [id],
      );
      const row = res.rows[0];
      if (!row) return { state: "missing" };
      if (!tokenMatches(pollToken, row.poll_token_sha256)) return { state: "forbidden" };
      if (row.expired) {
        await pool.query(
          "DELETE FROM explore_api.pair_sessions WHERE id = $1 AND expires_at <= now()",
          [id],
        );
        return { state: "expired" };
      }
      if (!row.completed) return { state: "pending" };
      // Releases the signature once: of two concurrent polls only one deletes the row.
      const released = await pool.query<{ completed_signer: string; completed_signature: string }>(
        `DELETE FROM explore_api.pair_sessions
          WHERE id = $1 AND completed_signer IS NOT NULL
          RETURNING completed_signer, completed_signature`,
        [id],
      );
      const r = released.rows[0];
      if (!r) return { state: "missing" };
      return { state: "completed", signer: r.completed_signer, signature: r.completed_signature };
    },

    async cancel(id, pollToken) {
      // The stored hash is compared to the hash of the presented token: the token never meets
      // the database in clear, and a byte-wise comparison of digests leaks nothing about it.
      const res = await pool.query(
        "DELETE FROM explore_api.pair_sessions WHERE id = $1 AND poll_token_sha256 = $2",
        [id, tokenHash(pollToken)],
      );
      return Boolean(res.rowCount);
    },
  };
}

let selected: PairStore | null = null;

/**
 * The PostgreSQL store when `CATALYST_DATABASE_URL` names the catalyst database (every deployment
 * with more than one replica must set it), otherwise one process's memory.
 */
function selectStore(): PairStore {
  if (!selected) {
    if (pairingDatabaseUrl()) {
      selected = createPgPairStore(pairingPool());
    } else {
      if (process.env.NODE_ENV === "production") {
        console.warn(
          "[pair] CATALYST_DATABASE_URL is not set: pairing sessions live in this process only",
        );
      }
      selected = createMemoryPairStore();
    }
  }
  return selected;
}

export const pairStore: PairStore = {
  create: (input) => selectStore().create(input),
  get: (id) => selectStore().get(id),
  complete: (id, signer, signature) => selectStore().complete(id, signer, signature),
  poll: (id, pollToken) => selectStore().poll(id, pollToken),
  cancel: (id, pollToken) => selectStore().cancel(id, pollToken),
};

const RATE_WINDOW_MS = 5 * 60_000;
const RATE_MAX_CREATES = 30;
const rateBuckets = new Map<string, { windowStart: number; count: number }>();

/**
 * The network a create is counted against: an IPv4 address itself, an IPv6 address by its /64
 * (one client usually holds a whole /64, so per address it could rotate past the limit), as the
 * catalyst's own per-network caps do.
 */
export function rateKey(ip: string): string {
  if (!ip.includes(":")) return ip;
  const mapped = /^::ffff:(\d+\.\d+\.\d+\.\d+)$/i.exec(ip);
  if (mapped) return mapped[1];
  const [head, tail = ""] = ip.toLowerCase().split("::", 2);
  const left = head ? head.split(":") : [];
  const right = tail ? tail.split(":") : [];
  const groups = [...left, ...Array(Math.max(0, 8 - left.length - right.length)).fill("0"), ...right];
  return `${groups.slice(0, 4).map((g) => g.replace(/^0+(?=.)/, "")).join(":")}::/64`;
}

export function allowPairCreate(ip: string, now = Date.now()): boolean {
  if (rateBuckets.size > 10_000) {
    for (const [key, b] of rateBuckets) {
      if (now - b.windowStart > RATE_WINDOW_MS) rateBuckets.delete(key);
    }
  }
  const key = rateKey(ip);
  const bucket = rateBuckets.get(key);
  if (!bucket || now - bucket.windowStart > RATE_WINDOW_MS) {
    rateBuckets.set(key, { windowStart: now, count: 1 });
    return true;
  }
  bucket.count += 1;
  return bucket.count <= RATE_MAX_CREATES;
}

// The drafts sites kept on its own disk before they moved to the node's
// builder projects (scene-drafts-client.ts). What is left reads them for
// the one-time move and deletes each once the node holds it; nothing
// writes here any more.
import { promises as fs } from "node:fs";
import path from "node:path";

import {
  AUTH_CHAIN_HEADER_PREFIX,
  AUTH_METADATA_HEADER,
  AUTH_TIMESTAMP_HEADER,
  buildRequestPayload,
} from "../../auth/signer";
import type { AuthLink } from "../../auth/types";
import {
  isSafeDraftId,
  isSafeWallet,
  normalizeWallet,
  type Draft,
  type DraftMeta,
} from "./scene-drafts";
import { ETH_ADDRESS_RE } from "../format/address";
import { catalystUrl } from "../bases";

export class DraftStoreUnavailable extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DraftStoreUnavailable";
  }
}

const UNAVAILABLE_CODES = new Set(["ENOENT", "ENOTDIR", "EACCES", "EPERM", "EROFS"]);

function unavailable(err: unknown, op: string): unknown {
  const code = (err as NodeJS.ErrnoException)?.code;
  if (!code || !UNAVAILABLE_CODES.has(code)) return err;
  return new DraftStoreUnavailable(
    `drafts store ${op} failed (${code}) under ${draftsRoot()}: set CH_DRAFTS_DIR (or SITES_STATE_DIR) to a readable directory`,
  );
}

type StoredFile = { meta: DraftMeta; blob: unknown };

export function draftsRoot(): string {
  if (process.env.CH_DRAFTS_DIR) return process.env.CH_DRAFTS_DIR;
  if (process.env.SITES_STATE_DIR) {
    return path.join(process.env.SITES_STATE_DIR, "ch-drafts");
  }
  return path.join(process.cwd(), "data", "ch-drafts");
}

/** The wallet's directory; `null` for an address no draft can live under. */
function walletDir(wallet: string): string | null {
  const w = normalizeWallet(wallet);
  return isSafeWallet(w) ? path.join(draftsRoot(), w) : null;
}

function draftPath(wallet: string, id: string): string | null {
  const dir = walletDir(wallet);
  return dir && isSafeDraftId(id) ? path.join(dir, `${id}.json`) : null;
}

async function readStored(file: string): Promise<StoredFile | null> {
  let raw: string;
  try {
    raw = await fs.readFile(file, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw unavailable(err, "read");
  }
  try {
    const parsed = JSON.parse(raw) as StoredFile;
    if (
      !parsed ||
      typeof parsed !== "object" ||
      !parsed.meta ||
      typeof parsed.meta.version !== "number"
    ) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export async function listDrafts(wallet: string): Promise<DraftMeta[]> {
  const dir = walletDir(wallet);
  if (!dir) return [];
  let names: string[];
  try {
    names = await fs.readdir(dir);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw unavailable(err, "list");
  }
  const metas: DraftMeta[] = [];
  for (const name of names) {
    if (!name.endsWith(".json")) continue;
    const id = name.slice(0, -5);
    if (!isSafeDraftId(id)) continue;
    const stored = await readStored(path.join(dir, name));
    if (stored?.meta) metas.push({ ...stored.meta, id });
  }
  metas.sort((a, b) => b.updatedAt - a.updatedAt);
  return metas;
}

export async function getDraft(
  wallet: string,
  id: string,
): Promise<Draft | null> {
  const file = draftPath(wallet, id);
  const stored = file ? await readStored(file) : null;
  if (!stored) return null;
  return { ...stored.meta, id, blob: stored.blob };
}

/** The node's answer for its creator plane (`GET /writes`). */
export type CreateWrites = { open: true } | { open: false; reason: string };

/** A hang guard: the node answers `/writes` from memory. */
const WRITES_STATUS_TIMEOUT_MS = 10_000;

/**
 * Whether the node takes creator writes now: `CREATE_WRITES_ENABLED` and the
 * restore stamps behind it (`GET /writes`). Deleting a draft here is local
 * authority the node's backups do not hold, so it follows the node's switch;
 * a node that cannot say is closed.
 */
export async function nodeCreateWrites(): Promise<CreateWrites> {
  const url = `${catalystUrl()}/writes`;
  try {
    const res = await fetch(url, {
      headers: { accept: "application/json" },
      signal: AbortSignal.timeout(WRITES_STATUS_TIMEOUT_MS),
    });
    if (!res.ok) return { open: false, reason: `the node's write status answered ${res.status}` };
    const body = (await res.json()) as { planes?: { create?: { open?: unknown; reason?: unknown } } };
    const create = body.planes?.create;
    if (create?.open === true) return { open: true };
    return {
      open: false,
      reason: typeof create?.reason === "string" ? create.reason : "the node reports no creator plane",
    };
  } catch (err) {
    return { open: false, reason: `the node's write status is unreachable (${(err as Error).message})` };
  }
}

export async function deleteDraft(wallet: string, id: string): Promise<void> {
  const file = draftPath(wallet, id);
  if (!file) return;
  try {
    await fs.unlink(file);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return;
    throw unavailable(err, "delete");
  }
}

type AuthResult =
  | { ok: true; wallet: string }
  | { ok: false; status: number; error: string };

const DEFAULT_MAX_AGE_MS = 10 * 60_000;

function parseEphemeral(
  message: string,
): { address: string; expiration: number } | null {
  const addr = /Ephemeral address:\s*(0x[0-9a-fA-F]{40})/i.exec(message);
  const exp = /Expiration:\s*(.+)\s*$/im.exec(message);
  if (!addr) return null;
  const expiration = exp ? Date.parse(exp[1].trim()) : NaN;
  return {
    address: addr[1].toLowerCase(),
    expiration: Number.isFinite(expiration) ? expiration : Infinity,
  };
}

export async function verifyAuthChainRequest(
  request: Request,
  opts: { now?: number; maxAgeMs?: number } = {},
): Promise<AuthResult> {
  const now = opts.now ?? Date.now();
  const maxAge = opts.maxAgeMs ?? DEFAULT_MAX_AGE_MS;

  const timestampRaw = request.headers.get(AUTH_TIMESTAMP_HEADER);
  const metadataStr = request.headers.get(AUTH_METADATA_HEADER) ?? "";
  if (!timestampRaw) {
    return { ok: false, status: 401, error: "missing identity timestamp" };
  }
  const timestamp = Number(timestampRaw);
  if (!Number.isFinite(timestamp)) {
    return { ok: false, status: 401, error: "invalid identity timestamp" };
  }
  if (Math.abs(now - timestamp) > maxAge) {
    return { ok: false, status: 401, error: "stale identity timestamp" };
  }

  const chain: AuthLink[] = [];
  for (let i = 0; ; i++) {
    const header = request.headers.get(`${AUTH_CHAIN_HEADER_PREFIX}${i}`);
    if (header === null) break;
    let link: AuthLink;
    try {
      link = JSON.parse(header) as AuthLink;
    } catch {
      return { ok: false, status: 401, error: `malformed auth link ${i}` };
    }
    if (
      !link ||
      typeof link.type !== "string" ||
      typeof link.payload !== "string" ||
      typeof link.signature !== "string"
    ) {
      return { ok: false, status: 401, error: `malformed auth link ${i}` };
    }
    chain.push(link);
    if (i > 8) return { ok: false, status: 401, error: "auth chain too long" };
  }
  if (chain.length < 2) {
    return { ok: false, status: 401, error: "empty auth chain" };
  }

  const signerLink = chain[0];
  if (signerLink.type !== "SIGNER" || !ETH_ADDRESS_RE.test(signerLink.payload)) {
    return { ok: false, status: 401, error: "missing SIGNER link" };
  }
  const owner = signerLink.payload.toLowerCase();

  const { recoverMessageAddress } = await import("viem");
  let authority = owner;
  let finalPayload: string | null = null;

  for (let i = 1; i < chain.length; i++) {
    const link = chain[i];
    let recovered: string;
    try {
      recovered = (
        await recoverMessageAddress({
          message: link.payload,
          signature: link.signature as `0x${string}`,
        })
      ).toLowerCase();
    } catch {
      return {
        ok: false,
        status: 401,
        error: `unrecoverable signature at link ${i}`,
      };
    }
    if (recovered !== authority) {
      return { ok: false, status: 401, error: `broken auth chain at link ${i}` };
    }

    const linkType: string = link.type;
    if (
      linkType === "ECDSA_EPHEMERAL" ||
      linkType === "ECDSA_PERSONAL_EPHEMERAL"
    ) {
      const eph = parseEphemeral(link.payload);
      if (!eph) {
        return { ok: false, status: 401, error: "malformed ephemeral payload" };
      }
      if (eph.expiration <= now) {
        return { ok: false, status: 401, error: "identity expired" };
      }
      authority = eph.address;
    } else if (
      linkType === "ECDSA_SIGNED_ENTITY" ||
      linkType === "ECDSA_PERSONAL_SIGNED_ENTITY"
    ) {
      finalPayload = link.payload;
    } else {
      return {
        ok: false,
        status: 401,
        error: `unsupported auth link type ${linkType}`,
      };
    }
  }

  if (finalPayload === null) {
    return { ok: false, status: 401, error: "missing signed request link" };
  }

  const rawMethod = request.method.toUpperCase();
  const method = rawMethod === "HEAD" ? "GET" : rawMethod;
  const path = new URL(request.url).pathname;
  const expected = buildRequestPayload(method, path, timestampRaw, metadataStr);
  if (finalPayload.toLowerCase() !== expected) {
    return {
      ok: false,
      status: 401,
      error: "request signature does not match request",
    };
  }

  return { ok: true, wallet: owner };
}

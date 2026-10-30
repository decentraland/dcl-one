// A recording: upstream answers keyed by (method, service, path, canonical query, request body),
// stored once and served to both sides. Replay never reaches the network; record mode reads the
// production service on a miss and keeps the raw answer (gitignored) for anonymize.mts.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { STATE } from "./paths.mts";
import { imageSize, placeholderPng } from "./png.mts";
import { canonicalQuery, ORIGINS, type Service } from "./services.mts";

export type Entry = {
  key: string;
  method: string;
  service: Service;
  path: string;
  query: string;
  /** The canonical request body a POST was keyed on (JSON), when it had one. */
  request?: unknown;
  status: number;
  contentType: string;
  /** File under bodies/, named by content hash. */
  body?: string;
  /** A third-party binary (a web font) kept out of git: fetched from url once, checked by sha256. */
  external?: { url: string; sha256: string };
};

export type Manifest = {
  set: string;
  recordedAt: string;
  source: string;
  reference: string;
  dclOne: string;
  /** Page targets discovered while recording (item URLs and the like), already anonymized. */
  targets: Record<string, string>;
  anonymization: string;
  entries: number;
};

export type Answer = { status: number; contentType: string; body: Buffer; headers?: Record<string, string> };

/** Sorted-key JSON, so two encodings of one request body hash alike. */
export function canonicalJson(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonicalJson).join(",")}]`;
  if (v && typeof v === "object") {
    return `{${Object.keys(v as object)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonicalJson((v as Record<string, unknown>)[k])}`)
      .join(",")}}`;
  }
  return JSON.stringify(v);
}

/** Google Fonts serves files by user agent; refetching uses the headless shell's own. */
const FONT_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/153.0.0.0 Safari/537.36";

const sha = (b: Buffer | string) => createHash("sha256").update(b).digest("hex");

export function entryKey(method: string, service: Service, path: string, query: string, request?: unknown): string {
  const base = `${method} ${service} ${path}${query ? `?${query}` : ""}`;
  return request === undefined ? base : `${base} #${sha(canonicalJson(request)).slice(0, 16)}`;
}

type RpcCall = { jsonrpc?: string; id?: unknown; method: string; params?: unknown };

function parseBody(body: Buffer | undefined): unknown {
  if (!body || body.length === 0) return undefined;
  try {
    return JSON.parse(body.toString("utf8"));
  } catch {
    return { raw: sha(body) };
  }
}

/**
 * Lookups the apps batch by id (and batch differently once the ids are anonymized, since they
 * sort them first): answered row by row from every recorded batch of the same route.
 */
type Batched = {
  service: Service;
  method: string;
  path: string;
  ids: (query: URLSearchParams, request: unknown) => string[];
  rows: (body: unknown) => unknown[];
  key: (row: unknown) => string | undefined;
  wrap: (rows: unknown[]) => unknown;
};

const field = (k: string) => (row: unknown) => {
  const v = (row as Record<string, unknown> | null)?.[k];
  return typeof v === "string" ? v.toLowerCase() : undefined;
};

const BATCHED: Batched[] = [
  {
    service: "marketplace",
    method: "GET",
    path: "/v3/catalog/items",
    ids: (q) => q.getAll("id").map((s) => s.toLowerCase()),
    rows: (b) => ((b as { data?: unknown[] }).data ?? []),
    key: field("id"),
    wrap: (rows) => ({ data: rows, total: rows.length }),
  },
  {
    service: "marketplace",
    method: "GET",
    path: "/v1/picks/stats",
    ids: (q) => q.getAll("itemId").map((s) => s.toLowerCase()),
    rows: (b) => ((b as { data?: unknown[] }).data ?? []),
    key: field("itemId"),
    wrap: (rows) => ({ ok: true, data: rows }),
  },
  {
    service: "peer",
    method: "POST",
    path: "/lambdas/profiles",
    ids: (_q, r) => (((r as { ids?: string[] } | undefined)?.ids ?? []) as string[]).map((s) => s.toLowerCase()),
    rows: (b) => (Array.isArray(b) ? b : b && typeof b === "object" && "avatars" in (b as object) ? [b] : []),
    key: (row) => {
      const a = (row as { avatars?: { ethAddress?: string; userId?: string }[] }).avatars?.[0];
      return (a?.ethAddress ?? a?.userId)?.toLowerCase();
    },
    wrap: (rows) => rows,
  },
];

export type Miss = { service: Service; method: string; path: string; query: string; side: string; request?: unknown };

export class Recording {
  readonly dir: string;
  readonly entries = new Map<string, Entry>();
  manifest: Manifest | null = null;
  readonly misses: Miss[] = [];
  readonly hits = new Map<string, Set<string>>();
  /** Intrinsic size of each content picture, by imageSeed; the replay paints placeholders this size. */
  readonly images = new Map<string, { width: number; height: number }>();
  /** dcl-one-only routes with no upstream (purchase offers): answered by this hook when recording. */
  synthesize: ((req: { method: string; service: Service; path: string; query: string; request: unknown }) => Promise<Answer | null>) | null = null;
  readonly live: boolean;

  constructor(dir: string, opts: { live?: boolean } = {}) {
    this.dir = dir;
    this.live = !!opts.live;
    const index = join(dir, "index.json");
    if (existsSync(index)) {
      for (const e of JSON.parse(readFileSync(index, "utf8")) as Entry[]) this.entries.set(e.key, e);
    }
    const manifest = join(dir, "manifest.json");
    if (existsSync(manifest)) this.manifest = JSON.parse(readFileSync(manifest, "utf8")) as Manifest;
    const images = join(dir, "images.json");
    if (existsSync(images)) for (const [k, v] of Object.entries(JSON.parse(readFileSync(images, "utf8")))) this.images.set(k, v as { width: number; height: number });
  }

  /** Record mode: read the real picture once to learn its size (its bytes are never kept). */
  async learnImage(seed: string, url: string): Promise<void> {
    if (!this.live || this.images.has(seed)) return;
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(20_000) });
      const size = imageSize(Buffer.from(await res.arrayBuffer()));
      if (size) this.images.set(seed, size);
    } catch {
      // unknown size: the placeholder keeps its default
    }
  }

  body(e: Entry): Buffer {
    if (e.external) {
      const file = join(STATE, "assets", e.external.sha256);
      if (!existsSync(file)) throw new Error(`asset ${e.external.url} is not fetched: run record.mts --assets`);
      return readFileSync(file);
    }
    return readFileSync(join(this.dir, "bodies", e.body!));
  }

  /** Fetch every external asset that is missing, refusing bytes that do not match the recording. */
  async fetchAssets(): Promise<number> {
    let fetched = 0;
    for (const e of this.entries.values()) {
      if (!e.external) continue;
      const file = join(STATE, "assets", e.external.sha256);
      if (existsSync(file)) continue;
      const res = await fetch(e.external.url, { headers: { "user-agent": FONT_UA }, signal: AbortSignal.timeout(30_000) });
      const bytes = Buffer.from(await res.arrayBuffer());
      const got = sha(bytes);
      if (got !== e.external.sha256) throw new Error(`${e.external.url}: sha256 ${got}, recorded ${e.external.sha256}`);
      mkdirSync(join(STATE, "assets"), { recursive: true });
      writeFileSync(file, bytes);
      fetched++;
    }
    return fetched;
  }

  putBody(e: Omit<Entry, "body" | "external">, body: Buffer): Entry {
    return this.store(e, body);
  }

  putExternal(e: Entry): Entry {
    this.entries.set(e.key, e);
    return e;
  }

  private note(key: string, side: string) {
    if (!this.hits.has(key)) this.hits.set(key, new Set());
    this.hits.get(key)!.add(side);
  }

  private async fetchLive(method: string, service: Service, path: string, query: string, body?: Buffer, headers?: Record<string, string>): Promise<Answer> {
    const origin = ORIGINS[service];
    if (!origin) return { status: 404, contentType: "application/json", body: Buffer.from(`{"error":"no upstream for ${service}"}`) };
    const url = `${origin}${path}${query ? `?${query}` : ""}`;
    const h: Record<string, string> = { accept: headers?.accept ?? "*/*" };
    if (headers?.["content-type"]) h["content-type"] = headers["content-type"];
    let lastError: unknown;
    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        const res = await fetch(url, { method, headers: h, body: method === "GET" || method === "HEAD" || !body ? undefined : new Uint8Array(body), signal: AbortSignal.timeout(30_000) });
        return { status: res.status, contentType: res.headers.get("content-type") ?? "application/octet-stream", body: Buffer.from(await res.arrayBuffer()) };
      } catch (err) {
        lastError = err;
      }
    }
    throw lastError;
  }

  private store(e: Omit<Entry, "body">, body: Buffer): Entry {
    const ext = /json/.test(e.contentType) ? "json" : /css/.test(e.contentType) ? "css" : /font|woff/.test(e.contentType) ? "woff2" : /html/.test(e.contentType) ? "html" : /javascript/.test(e.contentType) ? "js" : "bin";
    const name = `${sha(body).slice(0, 24)}.${ext}`;
    mkdirSync(join(this.dir, "bodies"), { recursive: true });
    writeFileSync(join(this.dir, "bodies", name), body);
    const entry = { ...e, body: name };
    this.entries.set(e.key, entry);
    return entry;
  }

  save(manifest?: Partial<Manifest>) {
    mkdirSync(this.dir, { recursive: true });
    const list = [...this.entries.values()].sort((a, b) => (a.key < b.key ? -1 : 1));
    writeFileSync(join(this.dir, "index.json"), `${JSON.stringify(list, null, 1)}\n`);
    const images = Object.fromEntries([...this.images.entries()].sort(([a], [b]) => (a < b ? -1 : 1)));
    writeFileSync(join(this.dir, "images.json"), `${JSON.stringify(images, null, 1)}\n`);
    if (manifest) {
      this.manifest = { ...(this.manifest ?? ({} as Manifest)), ...manifest, entries: list.length } as Manifest;
      writeFileSync(join(this.dir, "manifest.json"), `${JSON.stringify(this.manifest, null, 2)}\n`);
    }
  }

  private async rpc(path: string, body: Buffer | undefined, side: string): Promise<Answer> {
    const parsed = parseBody(body);
    const calls: RpcCall[] = Array.isArray(parsed) ? (parsed as RpcCall[]) : [parsed as RpcCall];
    const out: unknown[] = [];
    for (const call of calls) {
      const request = { method: call.method, params: call.params ?? [] };
      const key = entryKey("POST", "rpc", path, "", request);
      let entry = this.entries.get(key);
      if (!entry && this.live) {
        const res = await this.fetchLive("POST", "rpc", path, "", Buffer.from(JSON.stringify({ jsonrpc: "2.0", id: 1, ...request })), { "content-type": "application/json", accept: "application/json" });
        const answer = JSON.parse(res.body.toString("utf8")) as Record<string, unknown>;
        const result = Buffer.from(JSON.stringify("error" in answer ? { error: answer.error } : { result: answer.result }));
        entry = this.store({ key, method: "POST", service: "rpc", path, query: "", request, status: 200, contentType: "application/json" }, result);
      }
      if (!entry) {
        this.misses.push({ service: "rpc", method: "POST", path, query: "", side, request });
        out.push({ jsonrpc: "2.0", id: call.id ?? null, error: { code: -32000, message: "not recorded" } });
        continue;
      }
      this.note(key, side);
      out.push({ jsonrpc: "2.0", id: call.id ?? null, ...(JSON.parse(this.body(entry).toString("utf8")) as object) });
    }
    return { status: 200, contentType: "application/json", body: Buffer.from(JSON.stringify(Array.isArray(parsed) ? out : out[0])) };
  }

  private fromBatches(method: string, service: Service, path: string, query: string, request: unknown): Answer | null {
    const rule = BATCHED.find((b) => b.service === service && b.method === method && b.path === path);
    if (!rule) return null;
    const ids = rule.ids(new URLSearchParams(query), request);
    if (!ids.length) return null;
    const rest = [...new URLSearchParams(query)].filter(([k]) => k !== "id" && k !== "itemId" && k !== "first");
    const byId = new Map<string, unknown>();
    const asked = new Set<string>();
    for (const e of this.entries.values()) {
      if (e.service !== service || e.path !== path || e.method !== method || e.status !== 200) continue;
      // Other filters must agree; only the id list and its page size may differ.
      const eRest = [...new URLSearchParams(e.query)].filter(([k]) => k !== "id" && k !== "itemId" && k !== "first");
      if (JSON.stringify(eRest) !== JSON.stringify(rest)) continue;
      for (const id of rule.ids(new URLSearchParams(e.query), e.request)) asked.add(id);
      try {
        for (const row of rule.rows(JSON.parse(this.body(e).toString("utf8")))) {
          const k = rule.key(row);
          if (k && !byId.has(k)) byId.set(k, row);
        }
      } catch {
        // not JSON
      }
    }
    // Every id must have been asked while recording: answered, or known to be absent.
    if (ids.some((id) => !asked.has(id))) return null;
    const found = ids.map((id) => byId.get(id)).filter((r) => r !== undefined);
    return { status: 200, contentType: "application/json", body: Buffer.from(JSON.stringify(rule.wrap(found))) };
  }

  /** Answer one upstream request from the recording (or, in record mode, from the live service). */
  async answer(req: { method: string; service: Service; path: string; search: string; body?: Buffer; headers?: Record<string, string>; side: string }): Promise<Answer> {
    const method = req.method.toUpperCase();
    if (req.service === "rpc" && method === "POST") return this.rpc(req.path, req.body, req.side);
    const query = canonicalQuery(req.search);
    const request = method === "GET" || method === "HEAD" ? undefined : parseBody(req.body);
    const key = entryKey(method, req.service, req.path, query, request);
    let entry = this.entries.get(key);
    if (!entry && this.live && req.service === "offers" && this.synthesize) {
      const res = await this.synthesize({ method, service: req.service, path: req.path, query, request });
      if (res) entry = this.store({ key, method, service: req.service, path: req.path, query, ...(request === undefined ? {} : { request }), status: res.status, contentType: res.contentType }, res.body);
    }
    if (!entry && this.live && req.service !== "offers" && req.service !== "node") {
      const res = await this.fetchLive(method, req.service, req.path, query, req.body, req.headers);
      if (/^image\//.test(res.contentType)) {
        // A picture answered by an API route: keep its size only, never its bytes.
        const size = imageSize(res.body);
        if (size) this.images.set(req.path, size);
        return { status: 200, contentType: "image/png", body: placeholderPng(req.path, size?.width, size?.height) };
      }
      entry = this.store({ key, method, service: req.service, path: req.path, query, ...(request === undefined ? {} : { request }), status: res.status, contentType: res.contentType }, res.body);
    }
    if (!entry) {
      const batched = this.fromBatches(method, req.service, req.path, query, request);
      if (batched) {
        this.note(`${method} ${req.service} ${req.path} (by id)`, req.side);
        return batched;
      }
    }
    if (!entry && this.images.has(req.path)) {
      const size = this.images.get(req.path)!;
      return { status: 200, contentType: "image/png", body: placeholderPng(req.path, size.width, size.height) };
    }
    if (!entry) {
      this.misses.push({ service: req.service, method, path: req.path, query, side: req.side, ...(request === undefined ? {} : { request }) });
      return { status: 404, contentType: "application/json", body: Buffer.from(JSON.stringify({ ok: false, error: "not in the parity recording", key })) };
    }
    this.note(key, req.side);
    return { status: entry.status, contentType: entry.contentType, body: this.body(entry) };
  }
}

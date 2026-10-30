import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const WALLET = "0x7bbea9c18cd0541acab8c19da2b11d0c03faef1c";

vi.mock("../../auth/session", () => ({
  getIdentity: () => ({ signer: WALLET, ephemeral: {}, authChain: [] }),
}));
vi.mock("../../auth/signer", () => ({
  signRequest: async (_identity: unknown, method: string, path: string) => ({
    headers: { "x-signed": `${method} ${path}` },
  }),
}));

import {
  deleteServerDraft,
  draftManifest,
  draftProjectId,
  duplicateServerDraft,
  fetchServerDraft,
  forgetServerDraftVersions,
  listServerDrafts,
  manifestDraft,
  pushServerDraft,
  putServerDraft,
  renameServerDraft,
  uuidV5,
} from "./scene-drafts-client";
import { hashComposite } from "./scene-drafts";

// ── a fake node (the builder manifest API) and the old sites disk route ──

type Row = { manifest: Record<string, unknown> | null; title?: string; revision: number; deleted: boolean; updatedAt: string };
type Call = { method: string; url: string; headers: Record<string, string>; body: unknown };

let rows: Map<string, Row>;
let legacy: Map<string, { id: string; hash: string; version: number; updatedAt: number; title: string; blob: unknown }>;
let calls: Call[];
let refuse: ((call: Call) => Response | null) | null;

function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });
}

function projectRow(id: string, row: Row): Record<string, unknown> {
  const project = (row.manifest?.project ?? { title: row.title }) as Record<string, unknown>;
  return { ...project, id, eth_address: WALLET, manifest_revision: row.revision, updated_at: row.updatedAt };
}

function rendered(id: string, row: Row): Record<string, unknown> {
  return { ...row.manifest, project: projectRow(id, row) };
}

function node(call: Call): Response {
  const manifestPath = /^\/builder-api\/v1\/projects\/([0-9a-f-]{36})\/manifest$/.exec(call.url);
  const projectPath = /^\/builder-api\/v1\/projects\/([0-9a-f-]{36})$/.exec(call.url);
  if (call.method === "GET" && call.url === "/builder-api/v1/projects") {
    const items = [...rows].filter(([, r]) => !r.deleted).map(([id, r]) => projectRow(id, r));
    return json(200, { ok: true, data: { items, total: items.length } });
  }
  if (manifestPath && call.method === "GET") {
    const row = rows.get(manifestPath[1]!);
    if (!row?.manifest || row.deleted) return json(404, { ok: false, error: "Project not found" });
    return json(200, rendered(manifestPath[1]!, row), { etag: `"${row.revision}"` });
  }
  if (manifestPath && call.method === "PUT") {
    const id = manifestPath[1]!;
    const row = rows.get(id);
    const live = row?.manifest && !row.deleted ? row.revision : null;
    const ifMatch = call.headers["if-match"];
    const absent = call.headers["if-none-match"] === "*";
    const holds = absent ? live === null : ifMatch ? live !== null && ifMatch === `"${live}"` : true;
    if (!holds) {
      return json(412, {
        ok: false,
        error: "manifest_conflict",
        data: { revision: row?.revision ?? 0, manifest: live !== null ? rendered(id, row!) : null },
      });
    }
    const manifest = (call.body as { manifest: Record<string, unknown> }).manifest;
    const next: Row = { manifest, revision: (row?.revision ?? 0) + 1, deleted: false, updatedAt: new Date(1_000 * ((row?.revision ?? 0) + 1)).toISOString() };
    rows.set(id, next);
    return json(200, { ok: true, data: { id, manifest_revision: next.revision } }, { etag: `"${next.revision}"` });
  }
  if (projectPath && call.method === "DELETE") {
    const row = rows.get(projectPath[1]!);
    if (!row || row.deleted) return json(404, { ok: false, error: "Project not found" });
    row.deleted = true;
    return json(200, { ok: true, data: true });
  }
  const legacyPath = /^\/api\/creator-hub\/drafts(?:\/(.+))?$/.exec(call.url);
  if (legacyPath) {
    const id = legacyPath[1] ? decodeURIComponent(legacyPath[1]) : null;
    if (!id && call.method === "GET") return json(200, { drafts: [...legacy.values()].map(({ blob: _, ...meta }) => meta) });
    if (id && call.method === "GET") return legacy.has(id) ? json(200, legacy.get(id)) : json(404, { error: "draft not found" });
    if (id && call.method === "DELETE") {
      legacy.delete(id);
      return new Response(null, { status: 204 });
    }
  }
  throw new Error(`unexpected ${call.method} ${call.url}`);
}

beforeEach(() => {
  rows = new Map();
  legacy = new Map();
  calls = [];
  refuse = null;
  forgetServerDraftVersions();
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      const headers = Object.fromEntries(
        Object.entries((init?.headers ?? {}) as Record<string, string>).map(([k, v]) => [k.toLowerCase(), v]),
      );
      const body = typeof init?.body === "string" ? JSON.parse(init.body) : undefined;
      const call = { method: (init?.method ?? "GET").toUpperCase(), url, headers, body };
      calls.push(call);
      return refuse?.(call) ?? node(call);
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const sceneComposite = JSON.stringify({
  version: 1,
  components: [
    {
      name: "inspector::SceneMetadata-v3",
      data: { "0": { json: { name: "Plaza", layout: { base: { x: 10, y: 4 }, parcels: [{ x: 10, y: 4 }, { x: 11, y: 4 }, { x: 10, y: 5 }, { x: 11, y: 5 }, { x: 12, y: 5 }] } } } },
    },
    { name: "core::Transform", data: { "512": { json: { position: { x: 1, y: 2, z: 3 } } } } },
  ],
});
const blob = { composite: sceneComposite, title: "Plaza", base: "10,4", template: "empty", assets: { "assets/cat.glb": "bafkreicat" } };
const said = () => calls.map((c) => `${c.method} ${c.url}`);

// ── ids and the manifest shape ────────────────────────────────────────────

describe("draft project ids", () => {
  it("is RFC 4122 uuid v5, per wallet for a slug and the id itself for a uuid", async () => {
    expect(await uuidV5("6ba7b810-9dad-11d1-80b4-00c04fd430c8", "www.example.com")).toBe("2ed6657d-e927-568b-95e1-2665a8aea6a2");
    expect(await draftProjectId(WALLET, "untitled-scene")).toBe("33be387e-1196-5734-a58f-f7f2367c949b");
    expect(await draftProjectId(WALLET.toUpperCase().replace("0X", "0x"), "untitled-scene")).toBe("33be387e-1196-5734-a58f-f7f2367c949b");
    expect(await draftProjectId("0x1111111111111111111111111111111111111111", "untitled-scene")).not.toBe("33be387e-1196-5734-a58f-f7f2367c949b");
    expect(await draftProjectId(WALLET, "5F0B4F7E-3C2A-4D7B-9A10-0C2E4B6D8F01")).toBe("5f0b4f7e-3c2a-4d7b-9a10-0c2e4b6d8f01");
  });
});

describe("draft manifests", () => {
  it("hold a /create blob as an sdk7 scene sized by its layout, and read back to the same blob", () => {
    const manifest = draftManifest("plaza", "33be387e-1196-5734-a58f-f7f2367c949b", { hash: hashComposite(sceneComposite), blob });
    expect(manifest.project).toEqual({
      id: "33be387e-1196-5734-a58f-f7f2367c949b",
      title: "Plaza",
      description: "",
      scene_id: "33be387e-1196-5734-a58f-f7f2367c949b",
      cols: 3,
      rows: 2,
    });
    expect(manifest.scene).toEqual({
      sdk7: { id: "33be387e-1196-5734-a58f-f7f2367c949b", composite: JSON.parse(sceneComposite), mappings: blob.assets },
    });
    expect(manifest.draft).toEqual({ id: "plaza", hash: hashComposite(sceneComposite), base: "10,4", template: "empty" });

    const back = manifestDraft({ ...manifest, project: { ...manifest.project, manifest_revision: 4, updated_at: "2026-10-01T00:00:00.000Z" } });
    expect(back).toMatchObject({ id: "plaza", version: 4, title: "Plaza", updatedAt: Date.parse("2026-10-01T00:00:00.000Z") });
    expect(back!.blob).toEqual(blob);
  });

  it("keep any other engine blob whole, and skip an sdk6 builder scene", () => {
    const other = draftManifest("x", "33be387e-1196-5734-a58f-f7f2367c949b", { hash: "h", blob: { entities: [1] }, title: "X" });
    expect(other.scene).toEqual({});
    expect(manifestDraft(other)?.blob).toEqual({ entities: [1] });
    expect(manifestDraft({ version: 1, project: { id: "p", title: "Old" }, scene: { entities: {} } })).toBeNull();
  });
});

// ── saving ────────────────────────────────────────────────────────────────

describe("pushServerDraft", () => {
  it("creates under If-None-Match, then saves under the revision it wrote", async () => {
    const project = await draftProjectId(WALLET, "plaza");
    expect(await pushServerDraft("plaza", blob)).toBe(true);
    expect(said()).toEqual([
      "GET /api/creator-hub/drafts",
      "GET /builder-api/v1/projects",
      `PUT /builder-api/v1/projects/${project}/manifest`,
    ]);
    expect(calls[2]!.headers).toMatchObject({ "if-none-match": "*", "x-signed": `PUT /builder-api/v1/projects/${project}/manifest` });
    expect(rows.get(project)!.revision).toBe(1);

    expect(await pushServerDraft("plaza", { ...blob, title: "Plaza 2" })).toBe(true);
    expect(calls[3]!.headers["if-match"]).toBe('"1"');
    expect(rows.get(project)!.revision).toBe(2);
    expect((rows.get(project)!.manifest!.project as { title: string }).title).toBe("Plaza 2");
  });

  it("retries once at the node's revision after a 412, and reports false when the node refuses", async () => {
    expect(await pushServerDraft("plaza", blob)).toBe(true);
    const project = await draftProjectId(WALLET, "plaza");
    rows.get(project)!.revision = 7;
    expect(await pushServerDraft("plaza", blob)).toBe(true);
    expect(said().slice(-2)).toEqual([
      `PUT /builder-api/v1/projects/${project}/manifest`,
      `PUT /builder-api/v1/projects/${project}/manifest`,
    ]);
    expect(calls.at(-1)!.headers["if-match"]).toBe('"7"');
    expect(rows.get(project)!.revision).toBe(8);

    refuse = (call) => (call.method === "PUT" ? json(503, { ok: false, error: "writes are disabled" }) : null);
    expect(await pushServerDraft("plaza", blob)).toBe(false);
  });

  it("creates the draft again when another tab deleted it since the revision it knew", async () => {
    expect(await pushServerDraft("plaza", blob)).toBe(true);
    const project = await draftProjectId(WALLET, "plaza");
    rows.get(project)!.deleted = true;
    expect(await pushServerDraft("plaza", blob)).toBe(true);
    expect(calls.slice(-2).map((c) => c.headers["if-match"] ?? c.headers["if-none-match"])).toEqual(['"1"', "*"]);
    expect(rows.get(project)).toMatchObject({ deleted: false, revision: 2 });
  });
});

describe("putServerDraft (the sync engine's transport)", () => {
  it("answers the node's copy on a conflict instead of overwriting it", async () => {
    expect(await putServerDraft("plaza", { baseVersion: 0, hash: "h1", blob, title: "Plaza" })).toEqual({ ok: true, version: 1 });
    const stale = await putServerDraft("plaza", { baseVersion: 0, hash: "h2", blob: { ...blob, title: "Mine" } });
    expect(stale).toMatchObject({ ok: false, conflict: true, version: 1 });
    if (stale.ok || !stale.conflict) return;
    expect(stale.server?.blob).toEqual(blob);
    expect(await putServerDraft("plaza", { baseVersion: 1, hash: "h2", blob: { ...blob, title: "Mine" } })).toEqual({ ok: true, version: 2 });
  });
});

// ── reading and listing ───────────────────────────────────────────────────

describe("fetchServerDraft and listServerDrafts", () => {
  it("read a draft back under its slug, asking for no manifest the node does not list", async () => {
    expect(await fetchServerDraft("plaza")).toBeNull();
    expect(said()).toEqual(["GET /api/creator-hub/drafts", "GET /builder-api/v1/projects"]);
    await pushServerDraft("plaza", blob);
    const read = await fetchServerDraft("plaza");
    expect(read).toMatchObject({ id: "plaza", version: 1, title: "Plaza", blob });
    expect(read!.updatedAt).toBeGreaterThan(0);
  });

  it("list the projects holding a draft, under the local slug that names one and the project id otherwise", async () => {
    await pushServerDraft("plaza", blob);
    await pushServerDraft("market", { ...blob, title: "Market" });
    rows.set("0b9f0c1e-0000-4000-8000-000000000001", { manifest: null, title: "synced folder", revision: 0, deleted: false, updatedAt: new Date(0).toISOString() });
    const plaza = await draftProjectId(WALLET, "plaza");
    const market = await draftProjectId(WALLET, "market");
    const byId = (list: Awaited<ReturnType<typeof listServerDrafts>>) => Object.fromEntries((list ?? []).map((d) => [d.id, d.title]));
    expect(byId(await listServerDrafts(["plaza"]))).toEqual({ plaza: "Plaza", [market]: "Market" });
    expect(byId(await listServerDrafts())).toEqual({ [plaza]: "Plaza", [market]: "Market" });

    const elsewhere = await fetchServerDraft(market);
    expect(elsewhere).toMatchObject({ id: market, title: "Market", version: 1 });
    expect(await pushServerDraft(market, { ...blob, title: "Market 2" })).toBe(true);
    expect(rows.get(market)!.revision).toBe(2);
  });

  it("a listing the node could not answer rejects instead of reading as no drafts", async () => {
    expect(await listServerDrafts()).toEqual([]);
    const listing = (call: Call) => call.method === "GET" && call.url === "/builder-api/v1/projects";
    refuse = (call) => (listing(call) ? json(503, { ok: false, error: "unavailable" }) : null);
    await expect(listServerDrafts()).rejects.toThrow(/could not list your saved scenes \(503\)/);
    refuse = (call) => (listing(call) ? json(200, { ok: true }) : null);
    await expect(listServerDrafts()).rejects.toThrow(/without a list/);
    refuse = (call) => { if (listing(call)) throw new TypeError("Failed to fetch"); return null; };
    await expect(listServerDrafts()).rejects.toThrow("Failed to fetch");
    expect(await fetchServerDraft("plaza")).toBeNull();
  });
});

// ── managing ──────────────────────────────────────────────────────────────

describe("rename, duplicate and delete", () => {
  it("rename rewrites the title and the editor metadata under the revision it read", async () => {
    await pushServerDraft("plaza", blob);
    await renameServerDraft("plaza", "Market");
    const read = await fetchServerDraft("plaza");
    expect(read).toMatchObject({ version: 2, title: "Market" });
    const components = JSON.parse(read!.blob.composite).components;
    expect(components[0].data["0"].json.name).toBe("Market");
    expect(components[1]).toEqual(JSON.parse(sceneComposite).components[1]);
    await expect(renameServerDraft("missing", "X")).rejects.toThrow(/could not be renamed/);
  });

  it("duplicate writes a copy under a new uuid and leaves the original", async () => {
    await pushServerDraft("plaza", blob);
    const copy = await duplicateServerDraft("plaza");
    expect(copy.id).toMatch(/^[0-9a-f-]{36}$/);
    expect(copy.title).toBe("Plaza copy");
    expect(rows.get(copy.id)!.revision).toBe(1);
    expect((await fetchServerDraft(copy.id))?.blob.assets).toEqual(blob.assets);
    expect((await fetchServerDraft("plaza"))?.title).toBe("Plaza");
  });

  it("delete removes the project, and a later save starts it again", async () => {
    await pushServerDraft("plaza", blob);
    await deleteServerDraft("plaza");
    expect(await fetchServerDraft("plaza")).toBeNull();
    await deleteServerDraft("plaza");
    expect(await pushServerDraft("plaza", blob)).toBe(true);
    expect((await fetchServerDraft("plaza"))?.version).toBe(2);
  });
});

// ── the one-time move off the sites disk ──────────────────────────────────

describe("the legacy drafts import", () => {
  it("moves each disk draft to the node once, keeps the node's copy where one exists, and keeps what failed", async () => {
    await pushServerDraft("both", { ...blob, title: "Node copy" });
    legacy.set("old-plaza", { id: "old-plaza", hash: "h", version: 5, updatedAt: 1, title: "Old plaza", blob });
    legacy.set("both", { id: "both", hash: "h", version: 2, updatedAt: 1, title: "Disk copy", blob: { ...blob, title: "Disk copy" } });
    legacy.set("stuck", { id: "stuck", hash: "h", version: 1, updatedAt: 1, title: "Stuck", blob });
    const stuck = await draftProjectId(WALLET, "stuck");
    refuse = (call) => (call.method === "PUT" && call.url.includes(stuck) ? json(500, { ok: false }) : null);
    forgetServerDraftVersions();

    const listed = await listServerDrafts(["both", "old-plaza", "stuck"]);
    expect(listed?.map((d) => [d.id, d.title]).sort()).toEqual([["both", "Node copy"], ["old-plaza", "Old plaza"]]);
    expect([...legacy.keys()]).toEqual(["stuck"]);
    expect((await fetchServerDraft("old-plaza"))?.blob).toEqual({ ...blob, title: "Old plaza" });

    const before = calls.length;
    await listServerDrafts();
    expect(calls.slice(before).map((c) => `${c.method} ${c.url}`)).toEqual(["GET /builder-api/v1/projects"]);
  });
});

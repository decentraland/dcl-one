import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";

import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";

import { createIdentityFromPrivateKey } from "../../auth/identity";
import { signRequest } from "../../auth/signer";
import {
  action,
  loader,
} from "@/routes/routes/api.creator-hub.drafts.$";
import {
  DraftStoreUnavailable,
  deleteDraft,
  draftsRoot,
  getDraft,
  listDrafts,
  verifyAuthChainRequest,
} from "./scene-drafts.server";
import { hashComposite, renameComposite } from "./scene-drafts";

const A = "0x7bbea9c18cd0541acab8c19da2b11d0c03faef1c";
const B = "0x1111111111111111111111111111111111111111";

let dir: string;

/** The node's `GET /writes` as the fake node answers it; each test may change it. */
let createPlane: { open: boolean; reason: string | null };
let node: Server;
let nodeUrl: string;

beforeAll(async () => {
  node = createServer((req, res) => {
    if (req.url !== "/writes") {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ planes: { create: { switch: "CREATE_WRITES_ENABLED", on: true, ...createPlane } } }));
  });
  await new Promise<void>((resolve) => node.listen(0, "127.0.0.1", resolve));
  nodeUrl = `http://127.0.0.1:${(node.address() as AddressInfo).port}`;
});

afterAll(async () => {
  await new Promise<void>((resolve) => node.close(() => resolve()));
});

beforeEach(async () => {
  dir = await mkdtemp(path.join(tmpdir(), "ch-drafts-"));
  process.env.CH_DRAFTS_DIR = dir;
  process.env.CATALYST_URL = nodeUrl;
  createPlane = { open: true, reason: null };
});

afterEach(async () => {
  delete process.env.CH_DRAFTS_DIR;
  delete process.env.CATALYST_URL;
  await rm(dir, { recursive: true, force: true });
});

/** A draft as the old store wrote it: `<root>/<wallet>/<id>.json`. */
async function seed(wallet: string, id: string, blob: unknown, meta: { version?: number; updatedAt?: number; title?: string } = {}) {
  await mkdir(path.join(dir, wallet), { recursive: true });
  const stored = {
    meta: { id, hash: "h", version: meta.version ?? 1, updatedAt: meta.updatedAt ?? 1, title: meta.title ?? id },
    blob,
  };
  await writeFile(path.join(dir, wallet, `${id}.json`), JSON.stringify(stored), "utf8");
}

async function signed(
  key: `0x${string}`,
  method: string,
  pathname: string,
  body?: unknown,
  identityOpts: { expirationMs?: number } = {},
): Promise<{ request: Request; wallet: string }> {
  const identity = await createIdentityFromPrivateKey(key, identityOpts);
  const url = `http://localhost${pathname}`;
  const s = await signRequest(identity, method, pathname, {});
  const headers = new Headers(s.headers);
  if (body !== undefined) headers.set("content-type", "application/json");
  return {
    request: new Request(url, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    }),
    wallet: identity.signer,
  };
}

async function signedReq(
  key: `0x${string}`,
  method: string,
  pathname: string,
  body?: unknown,
): Promise<Request> {
  return (await signed(key, method, pathname, body)).request;
}

describe("legacy drafts store: what is left to move to the node", () => {
  it("lists the wallet's drafts newest first, reads one byte-equal, and deletes only the named one", async () => {
    const blob = {
      composite: JSON.stringify({ version: 1, components: [{ name: "Transform" }] }),
      nested: { arr: [true, null, "x", 3.14], deep: { k: "v" } },
    };
    await seed(A, "older", { v: 1 }, { updatedAt: 1 });
    await seed(A, "-86,78", blob, { updatedAt: 2, version: 3, title: "UAP" });
    await seed(B, "theirs", { v: 2 });
    await writeFile(path.join(dir, A, "not-a-draft.txt"), "x", "utf8");

    expect((await listDrafts(A)).map((d) => d.id)).toEqual(["-86,78", "older"]);
    expect((await listDrafts(B)).map((d) => d.id)).toEqual(["theirs"]);
    const read = await getDraft(A, "-86,78");
    expect(read).toMatchObject({ id: "-86,78", version: 3, title: "UAP" });
    expect(JSON.stringify(read!.blob)).toBe(JSON.stringify(blob));
    expect(await getDraft(A, "theirs")).toBeNull();

    await deleteDraft(A, "older");
    await deleteDraft(A, "older");
    expect((await listDrafts(A)).map((d) => d.id)).toEqual(["-86,78"]);
    expect(await getDraft(B, "theirs")).not.toBeNull();
  });

  it("never escapes the wallet directory and answers nothing for an invalid wallet", async () => {
    await seed(A, "kept", {});
    for (const id of ["../evil", "..", "a/b", "."]) {
      expect(await getDraft(A, id)).toBeNull();
      await deleteDraft(A, id);
    }
    expect(await listDrafts("0xshort")).toEqual([]);
    expect(await getDraft("0xshort", "x")).toBeNull();
    await deleteDraft("not-a-wallet", "kept");
    expect(await getDraft(A, "kept")).not.toBeNull();
  });

  it("resolves the root from CH_DRAFTS_DIR, then SITES_STATE_DIR, then cwd; an unreadable root is DraftStoreUnavailable and the route answers 503 with retry-after", async () => {
    expect(draftsRoot()).toBe(dir);
    delete process.env.CH_DRAFTS_DIR;
    process.env.SITES_STATE_DIR = "/var/lib/sites-state";
    expect(draftsRoot()).toBe(path.join("/var/lib/sites-state", "ch-drafts"));
    delete process.env.SITES_STATE_DIR;
    expect(draftsRoot()).toBe(path.join(process.cwd(), "data", "ch-drafts"));

    const blocker = path.join(dir, "blocker");
    await writeFile(blocker, "not a directory", "utf8");
    process.env.CH_DRAFTS_DIR = path.join(blocker, "ch-drafts");
    await expect(listDrafts(A)).rejects.toBeInstanceOf(DraftStoreUnavailable);
    await expect(getDraft(A, "scene")).rejects.toBeInstanceOf(DraftStoreUnavailable);

    const res = await loader({
      request: await signedReq(generatePrivateKey(), "GET", "/api/creator-hub/drafts"),
      params: { "*": "" },
    });
    expect(res.status).toBe(503);
    expect(Number(res.headers.get("retry-after"))).toBeGreaterThan(0);
    expect(((await res.json()) as { error: string }).error).toContain("CH_DRAFTS_DIR");
  });
});

describe("scene-drafts auth: ADR-44 AuthChain verification", () => {
  it("verifies a real signed request, derives the wallet from the chain, and rejects an unauthenticated request with 401 at the verifier and the route", async () => {
    const key = generatePrivateKey();
    const { request, wallet } = await signed(key, "GET", "/create/drafts");
    const res = await verifyAuthChainRequest(request);
    expect(res.ok).toBe(true);
    if (!res.ok) return;
    expect(res.wallet).toBe(wallet);
    expect(res.wallet).toBe(privateKeyToAccount(key).address.toLowerCase());

    const anon = await verifyAuthChainRequest(
      new Request("http://localhost/create/drafts"),
    );
    expect(anon.ok).toBe(false);
    if (anon.ok) return;
    expect(anon.status).toBe(401);

    const list = await loader({
      request: new Request("http://localhost/create/drafts"),
      params: { "*": "" },
    });
    expect(list.status).toBe(401);
  });

  it("rejects an expired ephemeral identity, a stale timestamp, and a signature replayed across paths with 401", async () => {
    const expiredKey = generatePrivateKey();
    const expired = await signed(expiredKey, "GET", "/create/drafts", undefined, {
      expirationMs: 1_000,
    });
    const expiredTs = Number(expired.request.headers.get("x-identity-timestamp"));
    const expiredRes = await verifyAuthChainRequest(expired.request, { now: expiredTs + 2_000 });
    expect(expiredRes.ok).toBe(false);
    if (expiredRes.ok) return;
    expect(expiredRes.status).toBe(401);
    expect(expiredRes.error).toContain("expired");

    const staleKey = generatePrivateKey();
    const stale = await signed(staleKey, "GET", "/create/drafts");
    const staleTs = Number(stale.request.headers.get("x-identity-timestamp"));
    const staleRes = await verifyAuthChainRequest(stale.request, { now: staleTs + 20 * 60_000 });
    expect(staleRes.ok).toBe(false);
    if (staleRes.ok) return;
    expect(staleRes.status).toBe(401);

    const replayKey = generatePrivateKey();
    const { request } = await signed(replayKey, "GET", "/create/drafts/one");
    const moved = new Request("http://localhost/create/drafts/two", {
      method: "GET",
      headers: request.headers,
    });
    const replayRes = await verifyAuthChainRequest(moved);
    expect(replayRes.ok).toBe(false);
    if (replayRes.ok) return;
    expect(replayRes.status).toBe(401);
  });
});

describe("legacy drafts route: read and delete for the move, no writes", () => {
  it("lists, reads and deletes the verified signer's drafts only", async () => {
    const key = generatePrivateKey();
    const wallet = privateKeyToAccount(key).address.toLowerCase();
    await seed(wallet, "mine", { composite: "{}" }, { title: "Mine" });
    await seed(B, "theirs", { secret: 42 });

    const list = await loader({ request: await signedReq(key, "GET", "/api/creator-hub/drafts"), params: { "*": "" } });
    expect(list.status).toBe(200);
    expect(((await list.json()) as { drafts: Array<{ id: string }> }).drafts.map((d) => d.id)).toEqual(["mine"]);

    const one = await loader({ request: await signedReq(key, "GET", "/api/creator-hub/drafts/mine"), params: { "*": "mine" } });
    expect(one.status).toBe(200);
    expect(await one.json()).toMatchObject({ id: "mine", title: "Mine", blob: { composite: "{}" } });

    const foreign = await loader({ request: await signedReq(key, "GET", "/api/creator-hub/drafts/theirs"), params: { "*": "theirs" } });
    expect(foreign.status).toBe(404);
    const foreignDelete = await action({ request: await signedReq(key, "DELETE", "/api/creator-hub/drafts/theirs"), params: { "*": "theirs" } });
    expect(foreignDelete.status).toBe(204);
    expect(await getDraft(B, "theirs")).not.toBeNull();

    const anonymous = await action({ request: new Request("http://localhost/api/creator-hub/drafts/mine", { method: "DELETE" }), params: { "*": "mine" } });
    expect(anonymous.status).toBe(401);
    const removed = await action({ request: await signedReq(key, "DELETE", "/api/creator-hub/drafts/mine"), params: { "*": "mine" } });
    expect(removed.status).toBe(204);
    expect(await getDraft(wallet, "mine")).toBeNull();
  });

  it("deletes only while the node takes creator writes, answering 503 with the node's reason", async () => {
    const key = generatePrivateKey();
    const wallet = privateKeyToAccount(key).address.toLowerCase();
    await seed(wallet, "kept", { v: 1 });
    const del = async () =>
      action({ request: await signedReq(key, "DELETE", "/api/creator-hub/drafts/kept"), params: { "*": "kept" } });

    createPlane = {
      open: false,
      reason: "the last complete backup (backup.json) is 31 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26",
    };
    const held = await del();
    expect(held.status).toBe(503);
    expect(held.headers.get("retry-after")).toBe("3600");
    expect(((await held.json()) as { error: string }).error).toContain("backup.json) is 31 h old");
    expect(await getDraft(wallet, "kept")).not.toBeNull();

    createPlane = { open: false, reason: "CREATE_WRITES_ENABLED=false" };
    const off = await del();
    expect(off.status).toBe(503);
    expect(((await off.json()) as { error: string }).error).toContain("CREATE_WRITES_ENABLED=false");

    process.env.CATALYST_URL = "http://127.0.0.1:1";
    const unreachable = await del();
    expect(unreachable.status).toBe(503);
    expect(((await unreachable.json()) as { error: string }).error).toContain("unreachable");
    expect(await getDraft(wallet, "kept")).not.toBeNull();

    process.env.CATALYST_URL = nodeUrl;
    createPlane = { open: true, reason: null };
    expect((await del()).status).toBe(204);
    expect(await getDraft(wallet, "kept")).toBeNull();
  });

  it("refuses every write with 405, naming the node's project API", async () => {
    const key = generatePrivateKey();
    const wallet = privateKeyToAccount(key).address.toLowerCase();
    await seed(wallet, "kept", { v: 1 });
    for (const method of ["PUT", "PATCH", "POST"]) {
      const res = await action({
        request: await signedReq(key, method, "/api/creator-hub/drafts/kept", { baseVersion: 1, hash: "h", blob: { v: 2 } }),
        params: { "*": "kept" },
      });
      expect(res.status).toBe(405);
      expect(res.headers.get("allow")).toBe("GET, DELETE");
      expect(((await res.json()) as { error: string }).error).toContain("/builder-api/v1/projects");
    }
    expect((await getDraft(wallet, "kept"))?.blob).toEqual({ v: 1 });
  });
});

describe("renameComposite", () => {
  it("renames embedded editor metadata without changing authored entities", () => {
    const components = [
      { name: "inspector::SceneMetadata-v3", data: { "0": { json: { name: "Original", description: "Keep this", spawnPoints: [{ name: "entry" }] } } } },
      { name: "core::Transform", data: { "512": { json: { position: { x: 1, y: 2, z: 3 } } } } },
    ];
    const composite = JSON.stringify({ version: 1, components });
    const renamed = renameComposite(composite, "Renamed");
    expect(JSON.parse(renamed).components).toEqual([
      { ...components[0], data: { "0": { json: { ...components[0].data["0"]!.json, name: "Renamed" } } } },
      components[1],
    ]);
    expect(hashComposite(renamed)).not.toBe(hashComposite(composite));
  });
});

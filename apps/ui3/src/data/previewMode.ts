import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { sendBridge } from "../overlay/bridge";
import { isPreviewShell } from "../overlay/nativeHost";

/** What the launch URL says about a scene preview; `assistant` asks for the Scene assistant panel
    to be open from the first frame (`assistant=1`, as `dcl-scene-cli start --talk` links it). */
export type PreviewLaunch = { preview: boolean; realm: string | null; origin: string | null; assistant: boolean };

/** The SDK run's capability token, handed over as `#token=` on the launch URL and kept only here:
    never in storage, and off the URL bar as soon as it is read, so a copied link or a screenshot
    carries no write access to the project. */
let token: string | null = null;

export function previewToken(): string | null {
  return token;
}

/** Forgets the claimed token (tests). */
export function forgetPreviewToken(): void {
  token = null;
}

/** Splits a location hash into the SDK token and the rest of the fragment (the MCP bridge's
    `mcptoken` rides the same hash and must survive). */
export function claimPreviewToken(hash: string): { token: string | null; rest: string } {
  const raw = hash.replace(/^#/, "");
  if (!raw) return { token: null, rest: "" };
  let params: URLSearchParams;
  try {
    params = new URLSearchParams(raw);
  } catch {
    return { token: null, rest: raw };
  }
  const found = params.get("token")?.trim() || null;
  if (found === null) return { token: null, rest: raw };
  params.delete("token");
  const rest = params.toString();
  token = found;
  return { token: found, rest };
}

function stripTokenFromUrl(rest: string): void {
  if (typeof window === "undefined" || typeof history === "undefined" || typeof history.replaceState !== "function") return;
  try {
    const { pathname, search } = window.location;
    history.replaceState(history.state, "", `${pathname}${search}${rest ? `#${rest}` : ""}`);
  } catch {}
}

export function previewLaunch(search: string, hash: string = typeof window === "undefined" ? "" : window.location.hash): PreviewLaunch {
  if (!isPreviewShell(search)) return { preview: false, realm: null, origin: null, assistant: false };
  const claimed = claimPreviewToken(hash);
  if (claimed.token) stripTokenFromUrl(claimed.rest);
  let realm: string | null = null;
  let assistant = false;
  try {
    const params = new URLSearchParams(search);
    realm = params.get("realm")?.trim() || null;
    assistant = /^(1|true|yes)$/i.test(params.get("assistant")?.trim() ?? "");
  } catch {
    realm = null;
  }
  // The engine's launch-params writer (engine/web/src/url-params.ts) drops `realm` from
  // the address bar when it is the bundle's default realm, which is the page's own origin: the
  // player dcl-scene-cli embeds is served by the SDK it previews. A preview with no realm left in
  // the URL is therefore a preview of this origin, not of nothing.
  if (!realm && typeof window !== "undefined" && /^https?:$/.test(window.location?.protocol ?? "")) realm = window.location.origin;
  let origin: string | null = null;
  if (realm) {
    try {
      const u = new URL(realm.replace(/\/about\/?$/, ""));
      if (/^https?:$/.test(u.protocol)) origin = u.origin;
    } catch {
      origin = null;
    }
  }
  return { preview: true, realm, origin, assistant: assistant && !!origin };
}

export function previewHost(origin: string | null): string {
  if (!origin) return "local scene";
  try {
    return new URL(origin).host;
  } catch {
    return origin;
  }
}

/** A spawn point of the previewed scene, in world metres and as a parcel. */
export type PreviewSpawn = { name: string; isDefault: boolean; parcel: [number, number]; world: { x: number; y: number; z: number } };
export type PreviewManifest = { title: string; base: [number, number]; parcels: number; spawns: PreviewSpawn[] };

function mid(v: unknown, fallback = 0): number {
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (Array.isArray(v) && v.length >= 2 && typeof v[0] === "number" && typeof v[1] === "number") return (v[0] + v[1]) / 2;
  return fallback;
}

function parseBase(v: unknown): [number, number] | null {
  const m = typeof v === "string" ? /^\s*(-?\d+)\s*,\s*(-?\d+)\s*$/.exec(v) : null;
  return m ? [Number(m[1]), Number(m[2])] : null;
}

/** Normalises a scene.json into what the preview HUD shows: SDK spawn positions are scene-relative
    metres (a number or a [min, max] range) from the base parcel's corner. */
export function toPreviewManifest(raw: unknown): PreviewManifest | null {
  if (!raw || typeof raw !== "object") return null;
  const json = raw as { display?: { title?: unknown }; scene?: { base?: unknown; parcels?: unknown }; spawnPoints?: unknown };
  const base = parseBase(json.scene?.base) ?? parseBase(Array.isArray(json.scene?.parcels) ? json.scene?.parcels[0] : null);
  if (!base) return null;
  const spawnsRaw = Array.isArray(json.spawnPoints) ? json.spawnPoints : [];
  const spawns: PreviewSpawn[] = spawnsRaw.flatMap((s, i) => {
    if (!s || typeof s !== "object") return [];
    const sp = s as { name?: unknown; default?: unknown; position?: { x?: unknown; y?: unknown; z?: unknown } };
    const px = mid(sp.position?.x, 8);
    const py = mid(sp.position?.y, 0);
    const pz = mid(sp.position?.z, 8);
    return [{
      name: typeof sp.name === "string" && sp.name.trim() ? sp.name.trim() : `Spawn ${i + 1}`,
      isDefault: sp.default === true,
      parcel: [base[0] + Math.floor(px / 16), base[1] + Math.floor(pz / 16)],
      world: { x: base[0] * 16 + px, y: py, z: base[1] * 16 + pz },
    }];
  });
  const title = typeof json.display?.title === "string" && json.display.title.trim() ? json.display.title.trim() : "Untitled scene";
  const parcels = Array.isArray(json.scene?.parcels) ? json.scene.parcels.length : 1;
  return { title, base, parcels, spawns };
}

export const MANIFEST_PATHS = ["/scene.json", "/scene-json"] as const;

export function useSceneManifest(origin: string | null) {
  return useQuery({
    queryKey: ["preview-manifest", origin],
    enabled: !!origin,
    staleTime: 30_000,
    retry: false,
    // Every preview server serves the project's scene.json at its root (upstream @dcl/sdk-commands
    // and dcl-scene-cli alike, with CORS); dcl-scene-cli's /scene-json is the fallback.
    queryFn: async ({ signal }) => {
      let last: unknown = null;
      for (const path of MANIFEST_PATHS) {
        try {
          const res = await fetch(`${origin}${path}`, { signal, headers: { accept: "application/json" } });
          if (!res.ok) { last = new Error(`${path} ${res.status}`); continue; }
          return toPreviewManifest(await res.json());
        } catch (error) {
          if (signal?.aborted) throw error;
          last = error;
        }
      }
      throw last instanceof Error ? last : new Error("scene manifest unavailable");
    },
  });
}

export function teleportToSpawn(spawn: PreviewSpawn): void {
  sendBridge("Teleport", { x: spawn.world.x, z: spawn.world.z });
}

/** Live link to the preview server: the same websocket the engine listens to for scene reloads. */
export type PreviewStatus = { connected: boolean | null; reloads: number; lastReloadAt: number | null };

export function isReloadFrame(data: unknown): boolean {
  if (typeof data !== "string") return false;
  try {
    const parsed = JSON.parse(data) as { type?: unknown };
    return parsed?.type === "SCENE_UPDATE";
  } catch {
    return false;
  }
}

export function previewSocketUrl(origin: string): string {
  return origin.replace(/^http/, "ws") + "/";
}

type Listener = (status: PreviewStatus) => void;
type Link = { status: PreviewStatus; listeners: Set<Listener>; socket: WebSocket | null; timer: ReturnType<typeof setTimeout> | null; teardown: ReturnType<typeof setTimeout> | null; attempt: number; closed: boolean };
const links = new Map<string, Link>();

/** How long a link outlives its last subscriber. React unmounts and remounts every effect once in
    development, and a layout switch does the same to the banner; the link waits that out instead
    of closing a socket that is still connecting and opening another. */
const LINK_GRACE_MS = 1000;

/** Close without the "closed before the connection is established" error a connecting socket
    reports, and without the reconnect its close handler would schedule. */
function closeSocket(socket: WebSocket | null): void {
  if (!socket) return;
  socket.onclose = null;
  socket.onerror = null;
  socket.onmessage = null;
  if (socket.readyState === 0) {
    socket.onopen = () => socket.close();
  } else {
    socket.onopen = null;
    socket.close();
  }
}

/** Close every link now, without the remount grace period. */
export function closePreviewLinks(): void {
  for (const link of links.values()) {
    link.closed = true;
    if (link.timer) clearTimeout(link.timer);
    if (link.teardown) clearTimeout(link.teardown);
    closeSocket(link.socket);
  }
  links.clear();
}

function openLink(origin: string, link: Link): void {
  if (link.closed) return;
  const update = (patch: Partial<PreviewStatus>) => {
    link.status = { ...link.status, ...patch };
    for (const l of link.listeners) l(link.status);
  };
  const schedule = () => {
    if (link.closed) return;
    link.attempt += 1;
    link.timer = setTimeout(() => openLink(origin, link), Math.min(10_000, 1000 * link.attempt));
  };
  let socket: WebSocket;
  try {
    socket = new WebSocket(previewSocketUrl(origin));
  } catch {
    schedule();
    return;
  }
  link.socket = socket;
  socket.onopen = () => { link.attempt = 0; update({ connected: true }); };
  socket.onmessage = (event) => {
    if (isReloadFrame(event.data)) update({ reloads: link.status.reloads + 1, lastReloadAt: Date.now() });
  };
  socket.onclose = () => { update({ connected: false }); schedule(); };
  socket.onerror = () => { socket.close(); };
}

/** Subscribes to the preview server's live link. One websocket per origin, opened on the first
    subscriber and closed after the last leaves, so the banner, the System section and the
    assistant share a connection instead of each holding their own. */
export function subscribePreviewStatus(origin: string, listener: Listener): () => void {
  let link = links.get(origin);
  if (!link) {
    link = { status: { connected: null, reloads: 0, lastReloadAt: null }, listeners: new Set(), socket: null, timer: null, teardown: null, attempt: 0, closed: false };
    links.set(origin, link);
    if (typeof WebSocket !== "undefined") openLink(origin, link);
  }
  if (link.teardown) {
    clearTimeout(link.teardown);
    link.teardown = null;
  }
  link.listeners.add(listener);
  listener(link.status);
  return () => {
    const current = links.get(origin);
    if (!current) return;
    current.listeners.delete(listener);
    if (current.listeners.size > 0 || current.teardown) return;
    current.teardown = setTimeout(() => {
      current.teardown = null;
      if (current.listeners.size > 0) return;
      current.closed = true;
      if (current.timer) clearTimeout(current.timer);
      closeSocket(current.socket);
      links.delete(origin);
    }, LINK_GRACE_MS);
  };
}

export function usePreviewStatus(origin: string | null): PreviewStatus {
  const [status, setStatus] = useState<PreviewStatus>({ connected: null, reloads: 0, lastReloadAt: null });
  useEffect(() => {
    if (!origin) return undefined;
    return subscribePreviewStatus(origin, setStatus);
  }, [origin]);
  return status;
}

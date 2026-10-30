import type {
  BusEnvelope,
  CameraMode,
  PageToSceneMessage,
  SceneToPageMessage,
  EditorTool,
} from "./bus-protocol";
import { EDITOR_BUS_CHANNEL } from "./bus-protocol";
import { SceneToPageMessageSchema } from "../generated/editor-bus-schemas";
import { RPC_TIMEOUT_MS, EXPORT_COMPOSITE_TIMEOUT_MS } from "./editor-config";

export { EDITOR_BUS_CHANNEL };

export const EDITOR_SESSION_PARAM = "editorSession";

const EDITOR_SESSION_RE = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/i;

export function editorBusChannel(session: string | null | undefined): string | null {
  return typeof session === "string" && EDITOR_SESSION_RE.test(session)
    ? `${EDITOR_BUS_CHANNEL}#${session}`
    : null;
}

// The viewport carries the session in its fragment, where the Explorer does not take it for a link
// parameter: the app seeds it as the sandbox's channel suffix (window.__bridgeSession), so the
// editor scene's bare `dcl-editor-bus` is this page's `dcl-editor-bus#<session>`.
function sessionFragment(url: URL): URLSearchParams {
  return new URLSearchParams(url.hash.slice(1));
}

export function viewportEditorSession(url: URL): string | null {
  return url.searchParams.get(EDITOR_SESSION_PARAM) ?? sessionFragment(url).get(EDITOR_SESSION_PARAM);
}

/** Put `session` wherever the viewport carries its session. */
export function setViewportEditorSession(url: URL, session: string): void {
  if (url.searchParams.has(EDITOR_SESSION_PARAM) || !sessionFragment(url).has(EDITOR_SESSION_PARAM)) {
    url.searchParams.set(EDITOR_SESSION_PARAM, session);
    return;
  }
  const fragment = sessionFragment(url);
  fragment.set(EDITOR_SESSION_PARAM, session);
  url.hash = fragment.toString();
}

/** Whether the viewport is the engine alone (`hud=0`, no editor `systemScene`): it shows the
    project, and with no editor scene at the other end of the bus nothing in it can be edited. */
export function isEngineOnlyViewport(viewportSrc: string | null | undefined): boolean {
  if (typeof viewportSrc !== "string" || viewportSrc === "") return false;
  try {
    const q = new URL(viewportSrc, globalThis.location?.origin ?? "https://example.org").searchParams;
    return q.get("hud") === "0" && !q.get("systemScene");
  } catch {
    return false;
  }
}

export function editorBusChannelFromViewportSrc(viewportSrc: string | null | undefined): string | null {
  if (typeof viewportSrc !== "string" || viewportSrc === "") return null;
  try {
    return editorBusChannel(viewportEditorSession(new URL(viewportSrc, globalThis.location?.origin ?? "https://example.org")));
  } catch {
    return null;
  }
}

export type EditorCamMode = "none" | "free" | "target";

export interface CameraInputDelta {
  orbitYaw?: number;
  orbitPitch?: number;
  panX?: number;
  panY?: number;
  dolly?: number;
}

export interface CameraSettingsInput {
  preset?: string;
  sensitivity?: { orbit?: number; pan?: number; zoom?: number };
  invertY?: boolean;
}

interface PendingRpc {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export interface EditorBus {
  ok: boolean;
  init(): void;
  setTool(tool: EditorTool): void;
  setFlags(flags: { orientGlobal?: boolean; pivotEach?: boolean }): void;
  setSelection(selected: string[], active: string | null): void;
  setComponent(entity: string | number | null | undefined, name: string, json: string): void;
  addComponent(entity: string | number | null | undefined, name: string): void;
  deleteComponent(entity: string | number | null | undefined, name: string): void;
  deleteEntity(entity: string | number | null | undefined, recursive?: boolean): void;
  setCamMode(mode: EditorCamMode): void;
  setCameraInput(d: CameraInputDelta | null | undefined): void;
  setCameraSettings(s: CameraSettingsInput | null | undefined): void;
  focus(entity: string | number | null | undefined, orbit?: boolean, satellite?: boolean): void;
  orientAxis(mode: EditorCamMode, axis: string): void;
  toggleOrtho(): void;
  addEntity(
    name?: string,
    parent?: number,
    components?: Record<string, unknown> | null,
    drop?: { x: number; y: number } | null,
  ): void;
  loadScene(composite: string, replace?: boolean): void;
  announcePlayState(playing: boolean, paused: boolean): void;
  rpc(method: string, args?: unknown[], timeoutMs?: number): Promise<unknown>;
  exportComposite(timeoutMs?: number): Promise<unknown>;
  onMessage(cb: (msg: SceneToPageMessage) => void): () => void;
  close(): void;
}

const NULL_BUS: EditorBus = {
  ok: false,
  init() {},
  setTool() {},
  setFlags() {},
  setSelection() {},
  setComponent() {},
  addComponent() {},
  deleteComponent() {},
  deleteEntity() {},
  addEntity() {},
  setCamMode() {},
  setCameraInput() {},
  setCameraSettings() {},
  focus() {},
  orientAxis() {},
  toggleOrtho() {},
  loadScene() {},
  announcePlayState() {},
  rpc() {
    return Promise.reject(new Error("editor bus unavailable"));
  },
  exportComposite() {
    return Promise.reject(new Error("editor bus unavailable"));
  },
  onMessage() {
    return () => {};
  },
  close() {},
};

const EDITOR_TOOLS = new Set<EditorTool>(["select", "translate", "rotate", "scale"]);
const EDITOR_CAM_MODES = new Set<EditorCamMode>(["none", "free", "target"]);

const toWireCamMode = (mode: EditorCamMode): CameraMode => (mode === "none" ? "off" : mode);

export function createEditorBus(viewportSrc?: string | null): EditorBus {
  if (typeof BroadcastChannel === "undefined") return NULL_BUS;
  const channelName = editorBusChannelFromViewportSrc(viewportSrc);
  if (!channelName) return NULL_BUS;

  let channel: BroadcastChannel;
  try {
    channel = new BroadcastChannel(channelName);
  } catch {
    return NULL_BUS;
  }

  const listeners = new Set<(msg: SceneToPageMessage) => void>();
  const pending = new Map<number | string, PendingRpc>();
  const token = Math.random().toString(36).slice(2, 10);
  let rpcSeq = 0;
  let closed = false;

  channel.onmessage = (ev: MessageEvent) => {
    const env = (ev?.data ?? null) as BusEnvelope | null;
    if (!env || typeof env !== "object" || env.to !== "page" || !env.msg) return;
    // This is an asynchronous cross-document boundary. A malformed/stale reply
    // must neither crash the event handler nor settle an unrelated request.
    const parsed = SceneToPageMessageSchema.safeParse(env.msg);
    if (!parsed.success) return;
    const msg = parsed.data;
    if (msg.type === "rpc-reply" && pending.has(msg.id)) {
      const entry = pending.get(msg.id);
      if (entry) {
        pending.delete(msg.id);
        if (entry.timer) clearTimeout(entry.timer);
        if (msg.ok) entry.resolve(msg.result);
        else entry.reject(new Error(msg.error || "rpc failed"));
      }
      return;
    }
    for (const cb of listeners) {
      try {
        cb(msg);
      } catch {
      }
    }
  };

  const post = (msg: PageToSceneMessage): Error | null => {
    if (closed) return new Error("editor bus closed");
    try {
      channel.postMessage({ to: "scene", msg } satisfies BusEnvelope);
      return null;
    } catch (error) {
      return error instanceof Error ? error : new Error("editor bus send failed");
    }
  };

  const bus: EditorBus = {
    get ok() { return !closed; },
    init() {
      post({ type: "init" });
    },
    setTool(tool) {
      if (EDITOR_TOOLS.has(tool)) post({ type: "set-tool", tool });
    },
    setSelection(selected, active) {
      post({
        type: "set-selection",
        selected: Array.isArray(selected) ? selected : [],
        active: active == null ? null : active,
      });
    },
    setFlags(flags) {
      if (!flags || typeof flags !== "object") return;
      const out: Extract<PageToSceneMessage, { type: "set-flags" }> = { type: "set-flags" };
      if (typeof flags.orientGlobal === "boolean") out.orientGlobal = flags.orientGlobal;
      if (typeof flags.pivotEach === "boolean") out.pivotEach = flags.pivotEach;
      if (Object.keys(out).length > 1) post(out);
    },
    setComponent(entity, name, json) {
      if (entity == null || !name || typeof json !== "string") return;
      post({ type: "set-component", entity: String(entity), name, json });
    },
    addComponent(entity, name) {
      if (entity == null || !name) return;
      post({ type: "add-component", entity: String(entity), name });
    },
    deleteComponent(entity, name) {
      if (entity == null || !name) return;
      post({ type: "delete-component", entity: String(entity), name });
    },
    deleteEntity(entity, recursive = true) {
      if (entity == null) return;
      post({ type: "entity-deleted", entity: String(entity), recursive: recursive !== false });
    },
    setCamMode(mode) {
      if (EDITOR_CAM_MODES.has(mode)) post({ type: "set-camera", mode: toWireCamMode(mode) });
    },
    setCameraInput(d) {
      if (!d || typeof d !== "object") return;
      const out: Extract<PageToSceneMessage, { type: "camera-input" }> = { type: "camera-input" };
      for (const k of ["orbitYaw", "orbitPitch", "panX", "panY", "dolly"] as const) {
        const v = d[k];
        if (typeof v === "number" && v !== 0) out[k] = v;
      }
      if (Object.keys(out).length > 1) post(out);
    },
    setCameraSettings(s) {
      if (!s || typeof s !== "object") return;
      const sens: { orbit?: number; pan?: number; zoom?: number } =
        s.sensitivity && typeof s.sensitivity === "object" ? s.sensitivity : {};
      post({
        type: "camera-settings",
        preset: s.preset === "maya" || s.preset === "blender-lmb" ? s.preset : "blender",
        sensitivity: {
          orbit: Number(sens.orbit) || 1,
          pan: Number(sens.pan) || 1,
          zoom: Number(sens.zoom) || 1,
        },
        invertY: !!s.invertY,
      });
    },
    focus(entity, orbit = true, satellite = false) {
      if (entity == null) return;
      post({
        type: "focus",
        entity: String(entity),
        orbit,
        ...(satellite ? { satellite: true } : {}),
      });
    },
    orientAxis(mode, axis) {
      if (EDITOR_CAM_MODES.has(mode) && typeof axis === "string") {
        post({ type: "set-camera", mode: toWireCamMode(mode), axis });
      }
    },
    toggleOrtho() {
      post({ type: "set-camera-projection", ortho: "toggle" });
    },
    addEntity(name, parent = 0, components = null, drop = null) {
      const hasDrop =
        drop && typeof drop === "object" &&
        Number.isFinite(drop.x) && Number.isFinite(drop.y);
      post({
        type: "add-entity",
        name: String(name || "Entity"),
        parent: Number(parent) || 0,
        components: components && typeof components === "object" ? components : null,
        ...(hasDrop ? { dropX: drop.x, dropY: drop.y } : {}),
      });
    },
    loadScene(composite, replace = false) {
      if (typeof composite !== "string" || composite.trim() === "") return;
      post({ type: "load-scene", composite, ...(replace ? { replace: true } : {}) });
    },
    announcePlayState(playing, paused) {
      const msg: SceneToPageMessage = { type: "play-state", playing: !!playing, paused: !!paused };
      try {
        channel.postMessage({ to: "page", msg } satisfies BusEnvelope);
      } catch {
      }
    },
    rpc(method, args = [], timeoutMs = RPC_TIMEOUT_MS) {
      return new Promise<unknown>((resolve, reject) => {
        if (closed) { reject(new Error("editor bus closed")); return; }
        const id = `${token}-${++rpcSeq}`;
        const timer = setTimeout(() => {
          if (pending.has(id)) {
            pending.delete(id);
            reject(new Error("rpc timeout: " + method));
          }
        }, timeoutMs);
        pending.set(id, { resolve, reject, timer });
        const failure = post({ type: "rpc", id, method, args });
        if (failure) {
          clearTimeout(timer);
          pending.delete(id);
          reject(failure);
        }
      });
    },
    exportComposite(timeoutMs = EXPORT_COMPOSITE_TIMEOUT_MS) {
      return bus.rpc("exportComposite", [], timeoutMs);
    },
    onMessage(cb) {
      if (closed || typeof cb !== "function") return () => {};
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    close() {
      if (closed) return;
      closed = true;
      listeners.clear();
      for (const { reject, timer } of pending.values()) {
        if (timer) clearTimeout(timer);
        try {
          reject(new Error("editor bus closed"));
        } catch {
        }
      }
      pending.clear();
      channel.onmessage = null;
      try {
        channel.close();
      } catch {
      }
    },
  };

  return bus;
}

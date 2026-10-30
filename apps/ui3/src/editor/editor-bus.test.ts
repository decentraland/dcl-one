import { afterEach, expect, it, vi } from "vitest";
import { createEditorBus, editorBusChannel, editorBusChannelFromViewportSrc, setViewportEditorSession } from "./editor-bus";

class Channel {
  static instances: Channel[] = [];
  onmessage: ((event: MessageEvent) => void) | null = null;
  closed = false;
  constructor(readonly name: string) { Channel.instances.push(this); }
  postMessage(data: unknown): void {
    for (const target of Channel.instances) {
      if (target !== this && !target.closed && target.name === this.name) target.onmessage?.({ data } as MessageEvent);
    }
  }
  close(): void { this.closed = true; }
}

afterEach(() => {
  Channel.instances = [];
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

it("derives distinct namespaced transports only from valid viewport sessions", () => {
  const a = "/_play/?editorUi=1&editorSession=00000000-0000-4000-8000-000000000001";
  const b = "/_play/?editorUi=1&editorSession=00000000-0000-4000-8000-000000000002";
  expect(editorBusChannelFromViewportSrc(a)).toBe("dcl-editor-bus#00000000-0000-4000-8000-000000000001");
  expect(editorBusChannelFromViewportSrc(b)).toBe("dcl-editor-bus#00000000-0000-4000-8000-000000000002");
  expect(editorBusChannelFromViewportSrc("/_play/?editorUi=1")).toBeNull();
  expect(editorBusChannel("not-a-session")).toBeNull();
});

it("finds and replaces the session where the viewport carries it, its launch params or its fragment", () => {
  const retry = "00000000-0000-4000-8000-000000000003";
  const query = new URL("https://sites.test/play/?editorSession=00000000-0000-4000-8000-000000000001");
  const fragment = new URL("https://sites.test/play/?hud=0#editorSession=00000000-0000-4000-8000-000000000002");
  expect(editorBusChannelFromViewportSrc(fragment.href)).toBe("dcl-editor-bus#00000000-0000-4000-8000-000000000002");
  setViewportEditorSession(query, retry);
  setViewportEditorSession(fragment, retry);
  expect(query.searchParams.get("editorSession")).toBe(retry);
  expect(fragment.searchParams.has("editorSession")).toBe(false);
  expect(editorBusChannelFromViewportSrc(fragment.href)).toBe(`dcl-editor-bus#${retry}`);
});

it("does not deliver one editor's messages to another editor namespace", () => {
  vi.stubGlobal("BroadcastChannel", Channel);
  const a = createEditorBus("/_play/?editorSession=00000000-0000-4000-8000-000000000001");
  const b = createEditorBus("/_play/?editorSession=00000000-0000-4000-8000-000000000002");
  const seenA = vi.fn();
  const seenB = vi.fn();
  a.onMessage(seenA);
  b.onMessage(seenB);
  const peer = new Channel("dcl-editor-bus#00000000-0000-4000-8000-000000000001");
  peer.postMessage({ to: "page", msg: { type: "tool", tool: "select" } });
  expect(seenA).toHaveBeenCalledOnce();
  expect(seenB).not.toHaveBeenCalled();
  a.close();
  b.close();
});

it("fails closed for a live viewport that has no session", () => {
  vi.stubGlobal("BroadcastChannel", Channel);
  expect(createEditorBus("/_play/?editorUi=1").ok).toBe(false);
  expect(createEditorBus().ok).toBe(false);
  expect(Channel.instances).toHaveLength(0);
});

const viewport = "/_play/?editorSession=00000000-0000-4000-8000-000000000001";

it("rejects pending and future RPCs immediately when a viewport is retired", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("BroadcastChannel", Channel);
  const bus = createEditorBus(viewport);
  const pending = expect(bus.rpc("exportComposite")).rejects.toThrow("editor bus closed");
  bus.close();
  bus.close();
  await pending;
  expect(bus.ok).toBe(false);
  await expect(bus.rpc("exportComposite")).rejects.toThrow("editor bus closed");
  expect(vi.getTimerCount()).toBe(0);
});

it("rejects failed sends without leaving a timeout behind", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("BroadcastChannel", Channel);
  const bus = createEditorBus(viewport);
  vi.spyOn(Channel.prototype, "postMessage").mockImplementation(() => { throw new Error("DataCloneError"); });
  await expect(bus.rpc("writeComponents", [() => {}])).rejects.toThrow("DataCloneError");
  expect(vi.getTimerCount()).toBe(0);
  bus.close();
});

it("ignores malformed messages and replies without breaking the next valid RPC", async () => {
  vi.stubGlobal("BroadcastChannel", Channel);
  const bus = createEditorBus(viewport);
  const listener = vi.fn();
  bus.onMessage(listener);
  const peer = new Channel(editorBusChannelFromViewportSrc(viewport)!);
  let id: string | undefined;
  peer.onmessage = event => { id = event.data.msg.id; };
  const result = bus.rpc("exportComposite");
  for (const msg of [null, "invalid", { type: "unknown" }, { type: "rpc-reply", id, ok: "yes" }]) {
    expect(() => peer.postMessage({ to: "page", msg })).not.toThrow();
  }
  expect(listener).not.toHaveBeenCalled();
  peer.postMessage({ to: "page", msg: { type: "rpc-reply", id, ok: true, result: "{}" } });
  await expect(result).resolves.toBe("{}");
  bus.close();
});

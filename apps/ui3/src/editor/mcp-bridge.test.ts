import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { autoConnect, connect } from "./mcp-bridge";

class FakeWebSocket {
  static urls: string[] = [];
  static instances: FakeWebSocket[] = [];
  static OPEN = 1;
  readyState = 0;
  onopen: (() => void) | null = null;
  onmessage: ((ev: unknown) => void) | null = null;
  onclose: ((ev: unknown) => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(url: string) {
    FakeWebSocket.urls.push(url);
    FakeWebSocket.instances.push(this);
  }
  send(): void {}
  close(): void {}
}

function setPageUrl(search: string, hash: string): void {
  window.history.replaceState(null, "", `/create/scene-editor${search}`);
  window.location.hash = hash;
}

let dispose: (() => void) | null = null;

function reset(): void {
  dispose?.();
  dispose = null;
  FakeWebSocket.urls = [];
  FakeWebSocket.instances = [];
  window.localStorage.clear();
}

beforeEach(() => {
  FakeWebSocket.urls = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
  if (typeof globalThis.BroadcastChannel === "undefined") {
    vi.stubGlobal(
      "BroadcastChannel",
      class {
        onmessage: unknown = null;
        postMessage(): void {}
        close(): void {}
      },
    );
  }
  window.localStorage.clear();
  document.body.innerHTML = '<iframe src="/_play/?editorUi=1&editorSession=00000000-0000-4000-8000-000000000001"></iframe>';
});

afterEach(() => {
  reset();
  document.body.innerHTML = "";
  vi.unstubAllGlobals();
  setPageUrl("", "");
});

it.each(["/play/", "/_play/"])("finds the editor's scoped relay on %s without an explicit accessor", path => {
  class ScopedChannel {
    static names: string[] = [];
    onmessage: unknown = null;
    constructor(name: string) { ScopedChannel.names.push(name); }
    postMessage(): void {}
    close(): void {}
  }
  vi.stubGlobal("BroadcastChannel", ScopedChannel);
  const viewport = document.querySelector("iframe")!;
  viewport.title = "Scene viewport";
  viewport.src = `${path}?realm=project&editorUi=1&editorSession=00000000-0000-4000-8000-000000000001`;
  dispose = connect({ url: "ws://127.0.0.1:8000/bridge", token: "scoped" });
  expect(ScopedChannel.names).toEqual(["dcl-editor-bus#00000000-0000-4000-8000-000000000001"]);
});

describe("mcp pairing gate", () => {
  it("reports confirmed pairing and server rejection to the SDK assistant", () => {
    const onPairingChange = vi.fn();
    dispose = connect({ url: "ws://127.0.0.1:8000/api/project/assistant/bridge", token: "sdk-project", onPairingChange });
    expect(onPairingChange).not.toHaveBeenCalled();
    const socket = FakeWebSocket.instances.at(-1)!;
    socket.onmessage?.({ data: JSON.stringify({ kind: "hello-ok", serverVersion: "1", heartbeatMs: 5000 }) });
    expect(onPairingChange).toHaveBeenLastCalledWith(true);
    socket.onclose?.({ code: 4409, reason: "Another editor is already paired" });
    expect(onPairingChange).toHaveBeenLastCalledWith(false, "Another editor is already paired");
  });
  it("pairs loopback silently: a bare port, the whole 127.0.0.0/8 block, and a stored loopback config", () => {
    setPageUrl("?mcp=5196", "#mcptoken=tok");
    const confirmRemote = vi.fn();
    dispose = autoConnect({ confirmRemote });
    expect(FakeWebSocket.urls).toEqual(["ws://127.0.0.1:5196/bridge"]);
    expect(confirmRemote).not.toHaveBeenCalled();
    reset();

    setPageUrl(`?mcp=${encodeURIComponent("ws://127.0.0.5:5196/bridge")}`, "#mcptoken=tok");
    dispose = autoConnect({ confirmRemote: vi.fn() });
    expect(FakeWebSocket.urls).toEqual(["ws://127.0.0.5:5196/bridge"]);
    reset();

    window.localStorage.setItem("dcl-mcp-relay", JSON.stringify({ url: 5196, token: "tok" }));
    setPageUrl("", "");
    dispose = autoConnect({ confirmRemote: vi.fn() });
    expect(FakeWebSocket.urls).toEqual(["ws://127.0.0.1:5196/bridge"]);
  });

  it("opens no socket for a remote relay until consent resolves, then connects to exactly the named relay only on approval", async () => {
    setPageUrl(`?mcp=${encodeURIComponent("wss://evil.example/bridge")}`, "#mcptoken=tok");
    let resolveConsent: ((ok: boolean) => void) | undefined;
    const confirmRemote = vi.fn(
      (host: string) =>
        new Promise<boolean>((resolve) => {
          void host;
          resolveConsent = resolve;
        }),
    );
    dispose = autoConnect({ confirmRemote });
    expect(confirmRemote).toHaveBeenCalledWith("evil.example");
    expect(FakeWebSocket.urls).toEqual([]);
    resolveConsent?.(false);
    await confirmRemote.mock.results[0]!.value;
    expect(FakeWebSocket.urls).toEqual([]);
    reset();

    setPageUrl(`?mcp=${encodeURIComponent("wss://relay.tail.example/bridge")}`, "#mcptoken=tok");
    const approval = Promise.resolve(true);
    dispose = autoConnect({ confirmRemote: () => approval });
    await approval;
    expect(FakeWebSocket.urls).toEqual(["wss://relay.tail.example/bridge"]);
  });

  it("refuses a remote relay with no consent surface, gates a stored remote config, and cancels a pairing disposed before consent", async () => {
    setPageUrl(`?mcp=${encodeURIComponent("wss://evil.example/bridge")}`, "#mcptoken=tok");
    dispose = autoConnect();
    expect(dispose).toBeNull();
    expect(FakeWebSocket.urls).toEqual([]);
    reset();

    window.localStorage.setItem(
      "dcl-mcp-relay",
      JSON.stringify({ url: "wss://far.example/bridge", token: "tok" }),
    );
    setPageUrl("", "");
    const confirmRemote = vi.fn(() => new Promise<boolean>(() => {}));
    dispose = autoConnect({ confirmRemote });
    expect(FakeWebSocket.urls).toEqual([]);
    expect(confirmRemote).toHaveBeenCalledWith("far.example");
    reset();

    setPageUrl(`?mcp=${encodeURIComponent("wss://slow.example/bridge")}`, "#mcptoken=tok");
    let resolveConsent: ((ok: boolean) => void) | undefined;
    const consent = new Promise<boolean>((resolve) => {
      resolveConsent = resolve;
    });
    dispose = autoConnect({ confirmRemote: () => consent });
    dispose?.();
    dispose = null;
    resolveConsent?.(true);
    await consent;
    expect(FakeWebSocket.urls).toEqual([]);
  });
});

it("binds its relay to the named target viewport transport", () => {
  class ScopedChannel {
    static names: string[] = [];
    onmessage: unknown = null;
    constructor(name: string) { ScopedChannel.names.push(name); }
    postMessage(): void {}
    close(): void {}
  }
  vi.stubGlobal("BroadcastChannel", ScopedChannel);
  const target = document.createElement("iframe");
  target.src = "/_play/?editorUi=1&editorSession=00000000-0000-4000-8000-0000000000aa";
  dispose = connect({
    url: "ws://127.0.0.1:8000/bridge",
    token: "targeted",
    getViewportEl: () => target,
  });
  expect(ScopedChannel.names).toEqual(["dcl-editor-bus#00000000-0000-4000-8000-0000000000aa"]);
});
